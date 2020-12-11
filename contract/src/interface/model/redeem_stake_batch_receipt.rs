use crate::interface::{EpochHeight, StakeTokenValue, YoctoNear, YoctoStake};
use crate::{
    domain,
    interface::{BatchId, TimestampedStakeBalance},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RedeemStakeBatchReceipt {
    redeemed_stake: YoctoStake,
    /// the value of the STAKE tokens that are being redeemed in this batch, which will be unstaked
    unstaked_near: YoctoNear,

    stake_token_value: StakeTokenValue,
}

impl RedeemStakeBatchReceipt {
    pub fn unstaked_near_withdrawal_availability(&self) -> EpochHeight {
        EpochHeight((self.stake_token_value.block_time_height.epoch_height.0 .0 + 4).into())
    }
}
