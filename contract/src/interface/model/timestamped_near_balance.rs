use crate::{
    domain,
    interface::{BlockHeight, BlockTimestamp, EpochHeight, YoctoNear},
};
use near_sdk::{
    json_types::U128,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TimestampedNearBalance {
    balance: YoctoNear,
    block_height: BlockHeight,
    block_timestamp: BlockTimestamp,
    epoch_height: EpochHeight,
}

impl From<domain::TimestampedNearBalance> for TimestampedNearBalance {
    fn from(balance: domain::TimestampedNearBalance) -> Self {
        Self {
            balance: balance.balance().into(),
            block_height: balance.block_height().into(),
            block_timestamp: balance.block_timestamp().into(),
            epoch_height: balance.epoch_height().into(),
        }
    }
}
