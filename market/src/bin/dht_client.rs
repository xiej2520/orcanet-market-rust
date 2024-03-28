use orcanet_market_ferrous::dht::DhtClient;

// simply spawns a dht client
#[tokio::main]
async fn main() -> Result<(), tokio::task::JoinError> {
    let (_dht_client, dht_handle) = match DhtClient::spawn_client() {
        Ok(o) => o,
        Err(err) => panic!("Failed to spawn DHT client: {err}"),
    };

    dht_handle.await
}
