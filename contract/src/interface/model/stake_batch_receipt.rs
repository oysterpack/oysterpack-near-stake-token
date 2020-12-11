use crate::interface::{StakeTokenValue, YoctoNear};
use crate::{
    domain,
    interface::{BatchId, TimestampedNearBalance},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeBatchReceipt {
    pub staked_near: YoctoNear,
    pub stake_token_value: StakeTokenValue,
}
