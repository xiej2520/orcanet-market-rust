use crate::*;

use std::collections::{hash_map, HashMap};
use std::error::Error;
use std::time::Duration;

use libp2p::futures::StreamExt;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{self, GetRecordError};
use libp2p::multiaddr::Protocol;
use libp2p::{
    mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use libp2p::{Multiaddr, PeerId, Swarm};

use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tonic::Status;

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<MemoryStore>,
    mdns: mdns::tokio::Behaviour,
}

/// Think about concurrency issues with multiple users accessing and modifying
/// the map at the same time later

async fn kad_node(mut swarm: Swarm<Behaviour>, mut rx_kad: mpsc::Receiver<Command>) {
    let mut pending_get: HashMap<String, Vec<_>> = HashMap::new();
    let mut pending_put: HashMap<String, Vec<_>> = HashMap::new();
    let mut pending_dial: HashMap<PeerId, mpsc::Sender<Result<PeerId, PeerId>>> = HashMap::new();

    loop {
        select! {
        // receive message and put into waiting map for response
        recv_msg = rx_kad.recv() => match recv_msg {
            Some(Command::GetRequests{key, resp}) => {
                swarm.behaviour_mut().kademlia.get_record(kad::RecordKey::new(&key));
                (*pending_get.entry(key).or_default()).push(resp);
            },
            Some(Command::Set{key, val, resp}) => {
                (*pending_put.entry(key.clone()).or_default()).push(resp);

                let key = kad::RecordKey::new(&key);
                let value = val.into_bytes();

                let record = kad::Record {
                    key,
                    value,
                    publisher: None,
                    expires: None,
                };
                swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One)
                    .expect("Failed to store record locally.");
            },
            Some(Command::Dial{peer_id, peer_addr, resp}) => {
                if let hash_map::Entry::Vacant(e) = pending_dial.entry(peer_id) {
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, peer_addr.clone());
                    match swarm.dial(peer_addr.with(Protocol::P2p(peer_id))) {
                        Ok(()) => {
                            println!("Dialing {peer_id}");
                            e.insert(resp);
                        }
                        Err(_) => {
                            let _ = resp.send(Err(peer_id));
                        }
                    }
                } else {
                    eprintln!("Already dialing {peer_id}");
                }
            }
            None => return,
        },
        // kad network event
        swarm_event = swarm.select_next_some() => match swarm_event {
            SwarmEvent::NewListenAddr { address, .. } => println!("Listening in {address:?}"),
            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, multiaddr) in list {
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. })) => {
                match result {
                    kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders { key, providers, .. })) => {
                        for peer in providers {
                            println!(
                                "Peer {peer:?} provides key {:?}",
                                std::str::from_utf8(key.as_ref()).unwrap()
                            );
                        }
                    }
                    kad::QueryResult::GetProviders(Err(err)) => {
                        eprintln!("Failed to get providers: {err:?}");
                    }
                    kad::QueryResult::GetRecord(Ok(
                        kad::GetRecordOk::FoundRecord(kad::PeerRecord {
                            record: kad::Record { key, value, .. },
                            ..
                        })
                    )) => {
                        println!(
                            "Got record {:?} {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap(),
                            std::str::from_utf8(&value).unwrap(),
                        );
                        let key_str = std::str::from_utf8(key.as_ref()).unwrap();

                        // wake up tasks that are waiting for response
                        for waiting in pending_get.get_mut(key_str).expect("Expected key in waiting map").drain(..) {
                            let value_str = std::str::from_utf8(value.as_ref()).unwrap().to_owned();
                            let requests = serde_json::from_str(&value_str).unwrap();
                            let _ = waiting.send(Ok(Some(requests)));
                        }
                    }
                    kad::QueryResult::GetRecord(Err(err)) => {
                        eprintln!("Failed to get record: {err:?}");
                        let key_str = std::str::from_utf8(err.key().as_ref()).unwrap();
                        match err {
                            GetRecordError::NotFound { .. } => {
                                for waiting in pending_get.get_mut(key_str).expect("Expected key in waiting map").drain(..) {
                                    let _ = waiting.send(Ok(None));
                                }
                            }
                            _ => {
                                for waiting in pending_get.get_mut(key_str).expect("Expected key in waiting map").drain(..) {
                                    let _ = waiting.send(Err(Status::unavailable("Failed to get record")));
                                }
                            }
                        }
                    }
                    kad::QueryResult::PutRecord(Ok(kad::PutRecordOk { key })) => {
                        println!(
                            "Successfully put record {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap()
                        );
                        // wake up tasks that are waiting for response
                        for waiting in pending_put.get_mut(std::str::from_utf8(key.as_ref()).unwrap()).expect("Expected key in waiting map").drain(..) {
                            let _ = waiting.send(Ok(()));
                        }
                    }
                    kad::QueryResult::PutRecord(Err(err)) => {
                        eprintln!("Failed to put record: {err:?}");
                        for waiting in pending_put.get_mut(std::str::from_utf8(err.key().as_ref()).unwrap()).expect("Expected key in waiting map").drain(..) {
                            let _ = waiting.send(Err(Status::unknown("Failed to put record")));
                        }
                    }
                    kad::QueryResult::StartProviding(Ok(kad::AddProviderOk { key })) => {
                        println!(
                            "Successfully put provider record {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap()
                        );
                    }
                    kad::QueryResult::StartProviding(Err(err)) => {
                        eprintln!("Failed to put provider record: {err:?}");
                    }
                    //kad::QueryResult::GetClosestPeers(_) => todo!(),
                    //kad::QueryResult::RepublishProvider(_) => todo!(),
                    //kad::QueryResult::RepublishRecord(_) => todo!(),
                    kad::QueryResult::Bootstrap(res) => {
                        eprintln!("{res:?}");
                    }
                    _ => {}
                }
            },
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                eprintln!("Successfully connected to {peer_id} at endpoint {endpoint:?}");
                if endpoint.is_dialer() {
                    if let Some(sender) = pending_dial.remove(&peer_id) {
                        let _ = sender.send(Ok(peer_id)).await;
                    }
                }
            },
            //SwarmEvent::ConnectionClosed { peer_id, connection_id, endpoint, num_established, cause } => todo!(),
            //SwarmEvent::IncomingConnection { connection_id, local_addr, send_back_addr } => todo!(),
            //SwarmEvent::IncomingConnectionError { connection_id, local_addr, send_back_addr, error } => todo!(),
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                eprintln!("Failed to connected to {peer_id:?} with error {error}");
                if let Some(peer_id) = peer_id {
                    if let Some(sender) = pending_dial.remove(&peer_id) {
                        let _ = sender.send(Err(peer_id));
                    }
                }
            },
            //SwarmEvent::ExpiredListenAddr { listener_id, address } => todo!(),
            //SwarmEvent::ListenerClosed { listener_id, addresses, reason } => todo!(),
            //SwarmEvent::ListenerError { listener_id, error } => todo!(),
            SwarmEvent::Dialing { peer_id: Some(peer_id), .. } => eprintln!("Dialing {peer_id}"),
            //SwarmEvent::NewExternalAddrCandidate { address } => todo!(),
            //SwarmEvent::ExternalAddrConfirmed { address } => todo!(),
            //SwarmEvent::ExternalAddrExpired { address } => todo!(),
            //_ => eprintln!("{swarm_event:?}"),
            _ => {},
        },
        }
    }
}

#[derive(Debug)]
pub enum Command {
    // file name -> Vec<FileRequest>
    GetRequests {
        key: String,
        resp: oneshot::Sender<Result<Option<Vec<FileRequest>>, Status>>,
    },
    Set {
        key: String,
        val: String,
        resp: oneshot::Sender<Result<(), Status>>,
    },
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        resp: mpsc::Sender<Result<PeerId, PeerId>>,
    }
}

#[derive(Debug, Clone)]
pub struct DhtClient {
    tx_kad: mpsc::Sender<Command>,
}
impl DhtClient {
    pub async fn spawn_client() -> Result<(Self, JoinHandle<()>), Box<dyn Error>> {
        //let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                //let config = kad::Config::default()
                Ok(Behaviour {
                    kademlia: kad::Behaviour::new(
                        key.public().to_peer_id(),
                        MemoryStore::new(key.public().to_peer_id()),
                    ),
                    mdns: mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        key.public().to_peer_id(),
                    )?,
                })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        // change to client for client node (less intensive)
        swarm.behaviour_mut().kademlia.set_mode(Some(kad::Mode::Server));

        let bootstrap_peers: Vec<Multiaddr> = vec![
            "/ip4/172.20.233.20/tcp/6881/p2p/QmbLpFuqficFLGNcQSCU8udEZLSTVSjpMBxVidWnLyEwXv"
                .parse()?,
        ];

        let (tx_kad, rx_kad) = mpsc::channel(256);
        
        let num_bootstrap = bootstrap_peers.len();
        let (tx_dial, mut rx_dial) = mpsc::channel(num_bootstrap);

        for peer_addr in bootstrap_peers {
            let Some(Protocol::P2p(peer_id)) = peer_addr.iter().last() else {
                return Err("Expect peer multiaddr to contain peer ID.".into());
            };
            eprintln!("Attempting to bootstrap with {peer_addr}");
            swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer_id, peer_addr.clone());

            // try dialing all peers in bootstrap
            let _ = tx_kad.send(Command::Dial { peer_id, peer_addr, resp: tx_dial.clone() }).await;
        }
        
        let handle = tokio::spawn(kad_node(swarm, rx_kad));
        
        // wait for dial results
        let time_limit = sleep(Duration::from_secs(1));
        tokio::pin!(time_limit);
        let mut connected_to_some = false;
        for _ in 0..num_bootstrap {
            select! {
                _ = &mut time_limit => {
                    if !connected_to_some {
                        return Err("Dialing bootstrap peers failed".into())
                    }
                }
                recv_msg = rx_dial.recv() => match recv_msg {
                    Some(Ok(peer_id)) => {
                        eprintln!("Successfully dialed bootstrap peer {peer_id}");
                        connected_to_some = true;
                    },
                    Some(Err(peer_id)) => {
                        eprintln!("Failed to dial {peer_id}");
                    },
                    None => return Err("Failed to receive dial result message".into()),
                }
            }
        }

        // automatically called now
        // swarm.behaviour_mut().kademlia.bootstrap()?;

        Ok((Self { tx_kad }, handle))
    }

    pub async fn get_requests(&self, file_hash: &str) -> Result<Option<Vec<FileRequest>>, Status> {
        let (tx, rx) = oneshot::channel();
        self.tx_kad
            .send(Command::GetRequests {
                key: file_hash.to_owned(),
                resp: tx,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn set_requests(
        &self,
        file_hash: &str,
        requests: Vec<FileRequest>,
    ) -> Result<(), Status> {
        let serialized = serde_json::to_string(&requests).map_err(|err| {
            eprintln!("{err}");
            Status::internal("Failed to serialize requests")
        })?;

        let (tx, rx) = oneshot::channel();

        self.tx_kad
            .send(Command::Set {
                key: file_hash.to_owned(),
                val: serialized,
                resp: tx,
            })
            .await
            .unwrap();

        rx.await.unwrap()
    }
}
