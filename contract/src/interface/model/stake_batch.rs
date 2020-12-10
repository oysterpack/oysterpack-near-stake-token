use crate::{
    domain,
    interface::{BatchId, TimestampedNearBalance},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeBatch {
    pub id: BatchId,
    pub balance: TimestampedNearBalance,
}

impl From<domain::StakeBatch> for StakeBatch {
    fn from(batch: domain::StakeBatch) -> Self {
        Self {
            id: BatchId(batch.id().0.into()),
            balance: batch.balance().into(),
        }
    }
}
