use crate::interface::{StakeTokenValue, YoctoNear};
use crate::{
    domain,
    interface::{BatchId},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeBatchReceipt {
    pub batch_id: BatchId,
    pub staked_near: YoctoNear,
    pub stake_token_value: StakeTokenValue,
}

impl StakeBatchReceipt {
    pub fn new(batch_id: domain::BatchId, receipt: domain::StakeBatchReceipt) -> Self {
        Self {
            batch_id: batch_id.into(),
            staked_near: receipt.staked_near().into(),
            stake_token_value: receipt.stake_token_value().into(),
        }
    }
}
