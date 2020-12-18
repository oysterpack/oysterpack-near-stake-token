use crate::interface::{BlockTimeHeight, YoctoNear, YoctoStake};
use crate::{
    domain,
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeTokenValue {
    pub block_time_height: BlockTimeHeight,
    pub total_staked_near_balance: YoctoNear,
    pub total_stake_supply: YoctoStake,
}

impl From<domain::StakeTokenValue> for StakeTokenValue {
    fn from(value: domain::StakeTokenValue) -> Self {
        Self {
            block_time_height: value.block_time_height().into(),
            total_staked_near_balance: value.total_staked_near_balance().into(),
            total_stake_supply: value.total_stake_supply().into(),
        }
    }
}
