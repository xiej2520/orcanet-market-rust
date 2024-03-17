use lib_proto::*;

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// time in secs that a file is valid for
pub const EXPIRATION_OFFSET: u64 = 3600;

// get the current time in seconds
pub fn get_current_time() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_secs()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileRequest {
    // apparently the field is not required in proto3? need to unwrap option
    pub user: User,
    pub file_hash: String,
    pub expiration: u64,
}

impl TryFrom<RegisterFileRequest> for FileRequest {
    type Error = ();
    // unwrap user
    fn try_from(req: RegisterFileRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            user: req.user.ok_or(())?,
            file_hash: req.file_hash,
            expiration: get_current_time() + EXPIRATION_OFFSET,
        })
    }
}
