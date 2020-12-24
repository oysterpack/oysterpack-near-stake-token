use crate::{
    domain,
    interface::{StakeTokenValue, YoctoNear},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeBatchReceipt {
    pub staked_near: YoctoNear,
    pub stake_token_value: StakeTokenValue,
}

impl From<domain::StakeBatchReceipt> for StakeBatchReceipt {
    fn from(receipt: domain::StakeBatchReceipt) -> Self {
        Self {
            staked_near: receipt.staked_near().into(),
            stake_token_value: receipt.stake_token_value().into(),
        }
    }
}
