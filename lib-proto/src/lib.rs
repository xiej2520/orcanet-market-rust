pub mod market {
    tonic::include_proto!("market"); // The string specified here must match the proto package name
}

pub use market::*;
pub use market_client::*;
pub use market_server::*;
