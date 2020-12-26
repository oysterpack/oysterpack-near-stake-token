use crate::interface::RedeemStakeBatchReceipt;
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
    /// if receipt is present it means the STAKE has been redeemed and the unstaked NEAR is still locked
    /// by the staking pool for withdrawal
    pub receipt: Option<RedeemStakeBatchReceipt>,
}

impl RedeemStakeBatch {
    pub fn from(batch: domain::RedeemStakeBatch, receipt: Option<RedeemStakeBatchReceipt>) -> Self {
        Self {
            id: BatchId(batch.id().0.into()),
            balance: batch.balance().into(),
            receipt,
        }
    }
}
