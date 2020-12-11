use crate::interface::{BlockTimeHeight, YoctoNear, YoctoStake};
use crate::{
    domain,
    interface::{BatchId, TimestampedNearBalance},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeTokenValue {
    pub block_time_height: BlockTimeHeight,
    pub total_staked_near_balance: YoctoNear,
    pub otal_stake_supply: YoctoStake,
}
