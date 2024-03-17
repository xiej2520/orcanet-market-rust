use orcanet_market_ferrous::*;

use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;

use libp2p::futures::StreamExt;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::Mode;
use libp2p::kad::{self, GetRecordError};
use libp2p::Swarm;
use libp2p::{
    mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};

use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tonic::Status;

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<MemoryStore>,
    mdns: mdns::tokio::Behaviour,
}

/// Think about concurrency issues with multiple users accessing and modifying
/// the map at the same time later

async fn kad_node(mut swarm: Swarm<Behaviour>, mut rx_kad: mpsc::Receiver<Command>) {
    let mut waiting_get: HashMap<String, Vec<_>> = HashMap::new();
    let mut waiting_put: HashMap<String, Vec<_>> = HashMap::new();
    loop {
        select! {
        // receive message and put into waiting map for response
        recv_msg = rx_kad.recv() => match recv_msg {
            Some(Command::GetRequests{key, resp}) => {
                swarm.behaviour_mut().kademlia.get_record(kad::RecordKey::new(&key));
                (*waiting_get.entry(key).or_default()).push(resp);
            },
            Some(Command::Set{key, val, resp}) => {
                (*waiting_put.entry(key.clone()).or_default()).push(resp);

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
                        for waiting in waiting_get.get_mut(key_str).expect("Expected key in waiting map").drain(..) {
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
                                for waiting in waiting_get.get_mut(key_str).expect("Expected key in waiting map").drain(..) {
                                    let _ = waiting.send(Ok(None));
                                }
                            }
                            _ => {
                                for waiting in waiting_get.get_mut(key_str).expect("Expected key in waiting map").drain(..) {
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
                        for waiting in waiting_put.get_mut(std::str::from_utf8(key.as_ref()).unwrap()).expect("Expected key in waiting map").drain(..) {
                            let _ = waiting.send(Ok(()));
                        }
                    }
                    kad::QueryResult::PutRecord(Err(err)) => {
                        eprintln!("Failed to put record: {err:?}");
                        for waiting in waiting_put.get_mut(std::str::from_utf8(err.key().as_ref()).unwrap()).expect("Expected key in waiting map").drain(..) {
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
                    //kad::QueryResult::Bootstrap(_) => todo!(),
                    _ => {}
                }
            },
            //SwarmEvent::ConnectionEstablished { peer_id, connection_id, endpoint, num_established, concurrent_dial_errors, established_in } => todo!(),
            //SwarmEvent::ConnectionClosed { peer_id, connection_id, endpoint, num_established, cause } => todo!(),
            //SwarmEvent::IncomingConnection { connection_id, local_addr, send_back_addr } => todo!(),
            //SwarmEvent::IncomingConnectionError { connection_id, local_addr, send_back_addr, error } => todo!(),
            //SwarmEvent::OutgoingConnectionError { connection_id, peer_id, error } => todo!(),
            //SwarmEvent::ExpiredListenAddr { listener_id, address } => todo!(),
            //SwarmEvent::ListenerClosed { listener_id, addresses, reason } => todo!(),
            //SwarmEvent::ListenerError { listener_id, error } => todo!(),
            //SwarmEvent::Dialing { peer_id, connection_id } => todo!(),
            //SwarmEvent::NewExternalAddrCandidate { address } => todo!(),
            //SwarmEvent::ExternalAddrConfirmed { address } => todo!(),
            //SwarmEvent::ExternalAddrExpired { address } => todo!(),
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
}

#[derive(Debug, Clone)]
pub struct KadWrapper {
    tx_kad: mpsc::Sender<Command>,
}
impl KadWrapper {
    pub fn spawn_kad() -> Result<(Self, JoinHandle<()>), Box<dyn Error>> {
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
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

        swarm.behaviour_mut().kademlia.set_mode(Some(Mode::Server));
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        let (tx_kad, rx_kad) = mpsc::channel(256);

        Ok((Self { tx_kad }, tokio::spawn(kad_node(swarm, rx_kad))))
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
