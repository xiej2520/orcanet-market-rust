use std::error::Error;

mod kad_wrapper;
use kad_wrapper::*;

mod market;
use market::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let argv: Vec<String> = std::env::args().collect();

    let (kad_wrapper, kad_handle) = match KadWrapper::spawn_kad() {
        Ok(o) => o,
        Err(err) => return Err(err),
    };

    let kw = kad_wrapper.clone();
    let server_handle = tokio::spawn(async move {
        if !argv.contains(&"-x".to_owned()) {
            let mut m = Server::new(kad_wrapper);
            m.server().await
        } else {
            println!("Skipped creating server");
            Ok(())
        }
    });

    let (server_res, kad_res) = (server_handle.await, kad_handle.await);
    server_res?.unwrap();
    kad_res?;

    // need to keep channel open to keep kad_handle alive, otherwise rx reads
    // None and it returns
    drop(kw);

    Ok(())
}
