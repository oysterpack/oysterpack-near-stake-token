use crate::{
    domain,
    interface::{EpochHeight, StakeTokenValue, YoctoStake},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RedeemStakeBatchReceipt {
    pub redeemed_stake: YoctoStake,

    pub stake_token_value: StakeTokenValue,
}

impl RedeemStakeBatchReceipt {
    pub fn unstaked_near_withdrawal_availability(&self) -> EpochHeight {
        EpochHeight((self.stake_token_value.block_time_height.epoch_height.0 .0 + 4).into())
    }
}

impl From<domain::RedeemStakeBatchReceipt> for RedeemStakeBatchReceipt {
    fn from(receipt: domain::RedeemStakeBatchReceipt) -> Self {
        Self {
            redeemed_stake: receipt.redeemed_stake().into(),
            stake_token_value: receipt.stake_token_value().into(),
        }
    }
}
