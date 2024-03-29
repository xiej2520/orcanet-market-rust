use clap::Parser;

use libp2p::{identity::Keypair, Multiaddr};
use orcanet_market_ferrous::dht::DhtClient;

/// To generate private keys
///
/// openssl genrsa -out private.pem 2048
/// openssl pkcs8 -in private.pem -inform PEM -topk8 -out private.pk8 -outform DER -nocrypt
///
/// rm private.pem      # optional

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
}

// simply spawns a dht client
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let bootstrap_peers = args.bootstrap_peers;

    let id_keys = if let Some(private_key) = args.private_key {
        let mut bytes = std::fs::read(private_key).expect("Failed to read private key bytes");
        let id_keys = Keypair::rsa_from_pkcs8(&mut bytes).expect("Failed to decode private key");
        println!("Peer Id: {}", id_keys.public().to_peer_id());
        Some(id_keys)
    } else {
        None
    };

    let listen_on = args.listen_address.zip(id_keys);

    let (_dht_client, dht_handle) = match DhtClient::spawn_client(&bootstrap_peers, listen_on).await
    {
        Ok(o) => o,
        Err(err) => panic!("Failed to spawn DHT client: {err:?}"),
    };

    dht_handle.await.map_err(|e| e.into())
}
