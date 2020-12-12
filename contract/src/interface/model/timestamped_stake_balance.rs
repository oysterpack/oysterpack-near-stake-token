use crate::{
    domain,
    interface::{BlockHeight, BlockTimestamp, EpochHeight, YoctoStake},
};
use near_sdk::{
    json_types::U128,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TimestampedStakeBalance {
    pub balance: YoctoStake,
    pub block_height: BlockHeight,
    pub block_timestamp: BlockTimestamp,
    pub epoch_height: EpochHeight,
}

impl From<domain::TimestampedStakeBalance> for TimestampedStakeBalance {
    fn from(balance: domain::TimestampedStakeBalance) -> Self {
        Self {
            balance: balance.balance().into(),
            block_height: balance.block_height().into(),
            block_timestamp: balance.block_timestamp().into(),
            epoch_height: balance.epoch_height().into(),
        }
    }
}
