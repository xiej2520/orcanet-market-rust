use std::error::Error;

use libp2p::Multiaddr;
use orcanet_market_ferrous::dht::DhtClient;
use orcanet_market_ferrous::market::Server;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Bootstrap nodes to connect to
    #[arg(short, long, required = true, num_args = 1..)]
    bootstrap_peers: Vec<Multiaddr>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let bootstrap_peers = args.bootstrap_peers;

    let (dht_client, dht_handle) = match DhtClient::spawn_client(&bootstrap_peers).await {
        Ok(o) => o,
        Err(err) => return Err(err),
    };

    let server_handle = tokio::spawn(async move {
        let mut m = Server::new(dht_client);
        m.server().await
    });

    let (server_res, kad_res) = (server_handle.await, dht_handle.await);
    server_res?.unwrap();
    kad_res?;

    Ok(())
}
