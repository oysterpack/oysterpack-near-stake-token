use crate::{
    domain,
    interface::{BlockHeight, BlockTimestamp, EpochHeight, YoctoNear},
};
use near_sdk::{
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TimestampedNearBalance {
    pub amount: YoctoNear,
    pub block_height: BlockHeight,
    pub block_timestamp: BlockTimestamp,
    pub epoch_height: EpochHeight,
}

impl From<domain::TimestampedNearBalance> for TimestampedNearBalance {
    fn from(balance: domain::TimestampedNearBalance) -> Self {
        Self {
            amount: balance.amount().into(),
            block_height: balance.block_height().into(),
            block_timestamp: balance.block_timestamp().into(),
            epoch_height: balance.epoch_height().into(),
        }
    }
}
