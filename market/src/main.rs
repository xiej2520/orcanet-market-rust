use std::error::Error;

use libp2p::identity::Keypair;
use libp2p::Multiaddr;
use orcanet_market_ferrous::dht::DhtClient;
use orcanet_market_ferrous::market::Server;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Bootstrap nodes to connect to
    #[arg(short, long, num_args = 0..)]
    bootstrap_peers: Vec<Multiaddr>,

    /// Private key file, should be in rsa pkcs8 format
    #[arg(short, long)]
    private_key: Option<String>,

    /// Multiaddr for listen address
    #[arg(short, long, default_value = "/ip4/0.0.0.0/tcp/6881")]
    listen_address: Option<Multiaddr>,

    // port the market server listens on
    #[arg(long, default_value = "50051")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let bootstrap_peers = args.bootstrap_peers;

    if args.port < 1024 {        
        Err("Invalid port")?;
    }

    let id_keys = if let Some(private_key) = args.private_key {
        let mut bytes = std::fs::read(private_key).expect("Failed to read private key bytes");
        let id_keys = Keypair::rsa_from_pkcs8(&mut bytes).expect("Failed to decode private key");
        println!("Peer Id: {}", id_keys.public().to_peer_id());
        Some(id_keys)
    } else {
        None
    };

    let listen_on = args.listen_address.zip(id_keys);

    let (dht_client, dht_handle) = match DhtClient::spawn_client(&bootstrap_peers, listen_on).await
    {
        Ok(o) => o,
        Err(err) => Err(err)?,
    };

    let server_handle = tokio::spawn(async move {
        let mut m = Server::new(dht_client, args.port);
        m.server().await
    });

    let (server_res, kad_res) = (server_handle.await, dht_handle.await);
    server_res?.unwrap();
    kad_res?;

    Ok(())
}
