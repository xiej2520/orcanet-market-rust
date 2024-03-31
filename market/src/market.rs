use lib_proto::*;

use crate::dht::DhtClient;
use crate::get_current_time;
use crate::FileRequest;

use std::error::Error;

use tonic::{Request, Response, Status};

#[derive(Clone, Debug)]
struct MarketData {
    kad_wrapper: DhtClient,
}

impl MarketData {
    fn new(kad_wrapper: DhtClient) -> Self {
        Self { kad_wrapper }
    }
    async fn insert_and_validate(&self, file_request: FileRequest) {
        let hash = file_request.file_hash.clone();
        let Ok(files) = self.kad_wrapper.get_requests(&hash).await else {
            eprintln!("Failed to fetch file requests from Kad");
            return;
        };
        let mut files = files.unwrap_or(vec![]);
        let current_time = get_current_time();
        files.retain(|holder| {
            holder.expiration >= current_time && holder.user.id != file_request.user.id
        });
        files.push(file_request);
        match self.kad_wrapper.set_requests(&hash, files).await {
            Ok(_) => {}
            Err(_) => eprintln!("Failed to update file requests in Kad"),
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
        // insert the file request into the market data and validate the holders
        self.market_data.insert_and_validate(file_request).await;
        Ok(Response::new(()))
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
    port: u16,
    //task_notify: Arc<Notify>,
}

impl Server {
    pub fn new(dht_client: DhtClient, port: u16) -> Self {
        let market_data = MarketData::new(dht_client);
        Self { market_data, port }
    }

    pub async fn server(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let ip = "127.0.0.1";
        let addr = format!("{}:{}", ip, self.port).parse()?;

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
