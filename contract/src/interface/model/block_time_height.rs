use crate::{
    domain,
    interface::{BlockHeight, BlockTimestamp, EpochHeight},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct BlockTimeHeight {
    pub block_height: BlockHeight,
    pub block_timestamp: BlockTimestamp,
    pub epoch_height: EpochHeight,
}

impl From<domain::BlockTimeHeight> for BlockTimeHeight {
    fn from(value: domain::BlockTimeHeight) -> Self {
        Self {
            block_height: value.block_height().into(),
            block_timestamp: value.block_timestamp().into(),
            epoch_height: value.epoch_height().into(),
        }
    }
}
