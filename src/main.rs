use std::collections::HashMap;
use std::sync::Arc;

use market::{CheckHoldersRequest, HoldersResponse, RegisterFileRequest, User};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use market::market_server::{Market, MarketServer};

pub mod market {
    tonic::include_proto!("market"); // The string specified here must match the proto package name
}

#[derive(Debug, Clone)]
struct FileRequest {
    // apparently the field is not required in proto3? need to unwrap option
    user: User,
    file_hash: String,
}

impl FileRequest {
    // unwrap user
    fn from(req: RegisterFileRequest) -> Result<Self, ()> {
        Ok(Self {
            user: req.user.ok_or(())?,
            file_hash: req.file_hash,
        })
    }
}
#[derive(Debug, Default)]
struct MarketData {
    files: HashMap<String, Vec<FileRequest>>,
}

impl MarketData {
    fn print_holders_map(&self) {
        for (hash, holders) in &self.files {
            println!("File Hash: {hash}");
            for holder in holders {
                let user = &holder.user;
                println!("Username: {}, Price: {}", user.name, user.price);
            }
        }
    }
}

// shared state for each rpc handler
// may be worth converting to channels/Actor model
#[derive(Debug)]
pub struct MarketState {
    market_data: Arc<Mutex<MarketData>>,
}

#[tonic::async_trait]
impl Market for MarketState {
    async fn register_file(
        &self,
        request: Request<RegisterFileRequest>,
    ) -> Result<Response<()>, Status> {
        let register_file_data = request.into_inner();
        let file_request = FileRequest::from(register_file_data)
            .map_err(|()| Status::invalid_argument("User not present"))?;
        let file_hash = file_request.file_hash.clone();

        let mut market_data = self.market_data.lock().await;

        (*market_data.files.entry(file_hash).or_default()).push(file_request);

        Ok(Response::new(()))
    }

    async fn check_holders(
        &self,
        request: Request<CheckHoldersRequest>,
    ) -> Result<Response<HoldersResponse>, Status> {
        let CheckHoldersRequest { file_hash } = request.into_inner();

        let market_data = self.market_data.lock().await;

        let mut users = vec![];
        let holders = market_data.files.get(&file_hash);

        if let Some(holders) = holders {
            for holder in holders {
                users.push(holder.user.clone());
            }
        }

        market_data.print_holders_map();

        Ok(Response::new(HoldersResponse { holders: users }))
    }
}

// instance of market server with data
struct Server {
    market_data: Arc<Mutex<MarketData>>,
    //task_notify: Arc<Notify>,
}

impl Server {
    fn new() -> Self {
        let market_data = Arc::new(Mutex::new(MarketData::default()));

        Self { market_data }
    }

    async fn server(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = "127.0.0.1:50051".parse()?;

        let market_state = MarketState {
            market_data: self.market_data.clone(),
        };

        tonic::transport::Server::builder()
            .add_service(MarketServer::new(market_state))
            .serve(addr)
            .await?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _argv: Vec<String> = std::env::args().collect();

    let mut m = Server::new();

    m.server().await?;
    println!("server launched");

    Ok(())
}
