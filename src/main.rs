use core::{hash, time};
use std::collections::HashMap;
use std::sync::Arc;

use market::{CheckHoldersRequest, HoldersResponse, RegisterFileRequest, User};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use market::market_server::{Market, MarketServer};

use orcanet_market_ferrous::{get_current_time, EXPIRATION_OFFSET};

pub mod market {
    tonic::include_proto!("market"); // The string specified here must match the proto package name
}

#[derive(Debug, Clone)]
struct FileRequest {
    // apparently the field is not required in proto3? need to unwrap option
    user: User,
    file_hash: String,
    expiration: u64,
}

impl FileRequest {
    // unwrap user
    fn from(req: RegisterFileRequest) -> Result<Self, ()> {
        Ok(Self {
            user: req.user.ok_or(())?,
            file_hash: req.file_hash,
            expiration: get_current_time() + EXPIRATION_OFFSET,
        })
    }
}
#[derive(Debug, Default)]
struct MarketData {
    files: HashMap<String, Vec<FileRequest>>,
}

impl MarketData {
    fn validate_holders(&self, hash: &String) -> Vec<FileRequest> {
        // make a map of holders already printed
        let mut previous_holders: HashMap<&String, &FileRequest> = HashMap::new();

        for holder in &self.files[hash] {
            let user = &holder.user;
            if previous_holders.contains_key(&user.id) {
                // check which holder has the most recent ttl
                // if the current holder has a more recent ttl, print it and remove the previous holder
                // if the previous holder has a more recent ttl, skip the current holder
                let prev_holder = previous_holders.get(&user.id).unwrap();
                let current_holder_ttl = holder.expiration - get_current_time(); // current holder: smaller means older
                let previous_holder_ttl: u64 = &prev_holder.expiration - get_current_time(); // previous holder: smaller means older

                if current_holder_ttl > previous_holder_ttl { // if the current holder is younger, remove the previous holder
                    previous_holders.insert(&user.id, &holder);
                }
                continue;
            }
            previous_holders.insert(&user.id, &holder);
        }
        return previous_holders.into_iter().map(|(_, holder)| holder.clone()).collect();
    }

    fn print_holders_map(&mut self) {
        let mut validated_files: HashMap<String, Vec<FileRequest>> = HashMap::new();

        for (hash, _) in &self.files {
            println!("File Hash: {hash}");
            let validated_holders = self.validate_holders(&hash);
            for holder in &validated_holders {
                let user = &holder.user;
                println!("Username: {}, Price: {}", user.name, user.price);
            }
            validated_files.insert(hash.clone(), validated_holders);
        }

        // set files to the validated files
        self.files = validated_files;
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

        let mut market_data = self.market_data.lock().await;
        let now = get_current_time();

        let mut users = vec![];

        let holders = market_data.files.get_mut(&file_hash);

        if let Some(holders) = holders {
            // check if any of the files have expired

            let mut first_valid = -1;
            //TODO: use binary search since times are inserted in order
            for (i, holder) in holders.iter().enumerate() {
                if holder.expiration > now {
                    first_valid = i as i32;
                    break;
                }
            }

            // no valid files, remove all of them
            if first_valid == -1 {
                println!("All files ({}) expired.", holders.len());
                market_data.files.remove(&file_hash);
            } else {
                if first_valid > 0 {
                    println!("Found {} expired files", first_valid);
                    // remove expired times
                    holders.drain(0..first_valid as usize);
                }

                for holder in holders {
                    users.push(holder.user.clone());
                }
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
