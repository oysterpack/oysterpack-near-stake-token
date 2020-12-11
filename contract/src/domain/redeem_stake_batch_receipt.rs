use crate::domain::{
    BlockTimeHeight, EpochHeight, StakeTokenValue, TimestampedNearBalance, TimestampedStakeBalance,
    YoctoNear, YoctoStake,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct RedeemStakeBatchReceipt {
    redeemed_stake: YoctoStake,
    /// the value of the STAKE tokens that are being redeemed in this batch, which will be unstaked
    unstaked_near: YoctoNear,

    stake_token_value: StakeTokenValue,
}

impl RedeemStakeBatchReceipt {
    pub fn unstaked_near_withdrawal_availability(&self) -> EpochHeight {
        self.stake_token_value.block_time_height().epoch_height() + 4
    }
}
