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
    fn insert_and_validate(&mut self, hash: &str, filerequest: &FileRequest) {
        // check if self.files[hash] exists
        if !self.files.contains_key(hash) {
            self.files
                .insert(hash.to_string(), vec![filerequest.clone()]);
        }
        let current_time = get_current_time();
        let producers = self.files.get_mut(hash).unwrap(); // safe to unwrap since we already checked if the key exists
        let mut index = 0;
        let mut len = producers.len();

        while index < len {
            let holder = producers.get(index).unwrap(); // safe to unwrap since index is always within bounds
            let user = &holder.user;
            // check if the user has expired
            if holder.expiration < current_time || user.id == filerequest.user.id {
                producers.swap_remove(index);
                len -= 1; // decrement length since we removed an element
                          // do not increment index since we just swapped the current index with the last element
                continue;
            } // this if statement must be first, otherwise it may unecessarily add expired holders or compare with expired holders

            index += 1;
        }
        producers.push(filerequest.clone());
    }

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

        // insert the file request into the market data and validate the holders
        market_data.insert_and_validate(&file_hash, &file_request);

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
