use clap::Parser;

use libp2p::Multiaddr;
use orcanet_market_ferrous::dht::DhtClient;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Bootstrap nodes to connect to
    #[arg(short, long, required = true, num_args = 1..)]
    bootstrap_peers: Vec<Multiaddr>,
}

// simply spawns a dht client
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let bootstrap_peers = args.bootstrap_peers;

    let (_dht_client, dht_handle) = match DhtClient::spawn_client(&bootstrap_peers).await {
        Ok(o) => o,
        Err(err) => panic!("Failed to spawn DHT client: {err}"),
    };

    dht_handle.await.map_err(|e| e.into())
}
