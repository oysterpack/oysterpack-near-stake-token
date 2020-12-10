use crate::{
    domain,
    interface::{BatchId, TimestampedStakeBalance},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RedeemStakeBatch {
    pub id: BatchId,
    pub balance: TimestampedStakeBalance,
}

impl From<domain::RedeemStakeBatch> for RedeemStakeBatch {
    fn from(batch: domain::RedeemStakeBatch) -> Self {
        Self {
            id: BatchId(batch.id().0.into()),
            balance: batch.balance().into(),
        }
    }
}
