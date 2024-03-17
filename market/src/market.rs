use crate::kad_wrapper::KadWrapper;
use lib_proto::*;
use orcanet_market_ferrous::FileRequest;
use orcanet_market_ferrous::get_current_time;

use std::error::Error;

use tonic::{Request, Response, Status};

#[derive(Clone, Debug)]
struct MarketData {
    kad_wrapper: KadWrapper,
}

impl MarketData {
    fn new(kad_wrapper: KadWrapper) -> Self {
        Self {
            kad_wrapper,
        }
    }
    //fn print_holders_map(&self) {
    //    for (hash, holders) in &self.files {
    //        println!("File Hash: {hash}");
    //        for holder in holders {
    //            let user = &holder.user;
    //            println!("Username: {}, Price: {}", user.name, user.price);
    //        }
    //    }
    //}
}

// shared state for each rpc handler
#[derive(Debug)]
pub struct MarketState {
    market_data: MarketData,
}

#[tonic::async_trait]
impl Market for MarketState {
    async fn register_file(
        &self,
        request: Request<RegisterFileRequest>,
    ) -> Result<Response<()>, Status> {
        let register_file_data = request.into_inner();
        let file_request = FileRequest::try_from(register_file_data)
            .map_err(|()| Status::invalid_argument("User not present"))?;
        let file_hash = file_request.file_hash.clone();

        // fetch requests from kad
        let requests = self
            .market_data
            .kad_wrapper
            .get_requests(&file_hash)
            .await?;

        let mut requests = requests.unwrap_or(vec![]);
        requests.push(file_request);

        self.market_data
            .kad_wrapper
            .set_requests(&file_hash, requests)
            .await
            .map(|_ok| Response::new(()))
    }

    async fn check_holders(
        &self,
        request: Request<CheckHoldersRequest>,
    ) -> Result<Response<HoldersResponse>, Status> {
        let CheckHoldersRequest { file_hash } = request.into_inner();

        let now = get_current_time();

        let mut users = vec![];

        let mut holders = self
            .market_data
            .kad_wrapper
            .get_requests(file_hash.as_str())
            .await?
            .unwrap_or(vec![]);

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
            //market_data.files.remove(&file_hash);
            holders.clear();
        } else {
            if first_valid > 0 {
                println!("Found {} expired files", first_valid);
                // remove expired times
                holders.drain(0..first_valid as usize);
            }

            for holder in holders.iter() {
                users.push(holder.user.clone());
            }
        }
        if let Err(err) = self
            .market_data
            .kad_wrapper
            .set_requests(file_hash.as_str(), holders)
            .await
        {
            eprintln!("Error: {:?}", err);
        }

        //market_data.print_holders_map();

        Ok(Response::new(HoldersResponse { holders: users }))
    }
}

// instance of market server with data
pub struct Server {
    market_data: MarketData,
    //task_notify: Arc<Notify>,
}

impl Server {
    pub fn new(kad_wrapper: KadWrapper) -> Self {
        let market_data = MarketData::new(kad_wrapper);

        Self { market_data }
    }

    pub async fn server(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
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
