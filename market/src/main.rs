use std::error::Error;

use orcanet_market_ferrous::dht::DhtClient;
use orcanet_market_ferrous::market::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //let argv: Vec<String> = std::env::args().collect();

    let (dht_client, dht_handle) = match DhtClient::spawn_client().await {
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
