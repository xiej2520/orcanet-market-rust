// starts a Kademlia network
use std::error::Error;
use std::time::Duration;

use clap::Parser;
use libp2p::futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::{kad, Multiaddr};
use libp2p::{
    mdns, noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<MemoryStore>,
    mdns: mdns::tokio::Behaviour,
}

/// To generate private keys
///
/// openssl genrsa -out private.pem 2048
/// openssl pkcs8 -in private.pem -inform PEM -topk8 -out private.pk8 -outform DER -nocrypt
///
/// rm private.pem      # optional

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Private key file, should be in rsa pkcs8 format
    #[arg(short, long, default_value = "private.pk8")]
    private_key: String,

    /// Multiaddr for listen address
    #[arg(short, long, default_value = "/ip4/0.0.0.0/tcp/6881")]
    listen_address: Multiaddr,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut bytes = std::fs::read(args.private_key).expect("Failed to read private key bytes");

    let id_keys = Keypair::rsa_from_pkcs8(&mut bytes).expect("Failed to decode private key");
    let peer_id = id_keys.public().to_peer_id();
    println!("Peer Id: {peer_id}");

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
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

    swarm
        .behaviour_mut()
        .kademlia
        .set_mode(Some(kad::Mode::Server));
    swarm.listen_on(args.listen_address)?;
    println!("DHT swarm started");

    loop {
        let event = swarm.select_next_some().await;
        match event {
            SwarmEvent::NewListenAddr { address, .. } => println!("Listening in {address:?}"),
            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, multiaddr) in list {
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, multiaddr);
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed { result, .. },
            )) => match result {
                kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
                    key,
                    providers,
                    ..
                })) => {
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
                kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(
                    kad::PeerRecord {
                        record: kad::Record { key, value, .. },
                        ..
                    },
                ))) => {
                    println!(
                        "Got record {:?} {:?}",
                        std::str::from_utf8(key.as_ref()).unwrap(),
                        std::str::from_utf8(&value).unwrap(),
                    );
                }
                kad::QueryResult::GetRecord(Ok(_)) => {}
                kad::QueryResult::GetRecord(Err(err)) => {
                    eprintln!("Failed to get record: {err:?}");
                }
                kad::QueryResult::PutRecord(Ok(kad::PutRecordOk { key })) => {
                    println!(
                        "Successfully put record {:?}",
                        std::str::from_utf8(key.as_ref()).unwrap()
                    );
                }
                kad::QueryResult::PutRecord(Err(err)) => {
                    eprintln!("Failed to put record: {err:?}");
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
                _ => {}
            },
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                eprintln!("Successfully connected to {peer_id} at endpoint {endpoint:?}");
            },
            SwarmEvent::ConnectionClosed { peer_id, .. } => eprintln!("Connected to {peer_id} closed"),
            SwarmEvent::IncomingConnection { local_addr, send_back_addr, .. } =>
                eprintln!("Incoming connection local addr: {local_addr}, send_back_addr {send_back_addr}"),
            SwarmEvent::IncomingConnectionError { local_addr, send_back_addr, error, .. } =>
                eprintln!("Incoming connection error: {error}, local_addr: {local_addr}, send_back_addr: {send_back_addr}"),
            _ => eprintln!("{event:?}"),
        };
    }
}
