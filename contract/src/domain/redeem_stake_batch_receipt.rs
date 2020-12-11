use crate::domain::{
    BlockTimeHeight, EpochHeight, StakeTokenValue, TimestampedNearBalance, TimestampedStakeBalance,
    YoctoNear, YoctoStake,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct RedeemStakeBatchReceipt {
    redeemed_stake: YoctoStake,
    stake_token_value: StakeTokenValue,
}

impl RedeemStakeBatchReceipt {
    pub fn stake_token_value(&self) -> StakeTokenValue {
        self.stake_token_value
    }

    pub fn unstaked_near_withdrawal_availability(&self) -> EpochHeight {
        self.stake_token_value.block_time_height().epoch_height() + 4
    }

    /// Used to track when an account has claimed their STAKE tokens for the NEAR they have staked
    pub fn stake_tokens_redeemed(&mut self, redeemed_stake: YoctoStake) {
        self.redeemed_stake -= redeemed_stake;
    }

    pub fn all_claimed(&self) -> bool {
        self.redeemed_stake.value() == 0
    }
}
