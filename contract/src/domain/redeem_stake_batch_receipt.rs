use crate::{
    domain::{
        BlockTimeHeight, EpochHeight, StakeTokenValue, TimestampedNearBalance,
        TimestampedStakeBalance, YoctoNear, YoctoStake,
    },
    near::UNSTAKED_NEAR_FUNDS_NUM_EPOCHS_TO_UNLOCK,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct RedeemStakeBatchReceipt {
    redeemed_stake: YoctoStake,
    stake_token_value: StakeTokenValue,
}

impl RedeemStakeBatchReceipt {
    pub fn new(redeemed_stake: YoctoStake, stake_token_value: StakeTokenValue) -> Self {
        Self {
            redeemed_stake,
            stake_token_value,
        }
    }

    pub fn redeemed_stake(&self) -> YoctoStake {
        self.redeemed_stake
    }

    pub fn stake_token_value(&self) -> StakeTokenValue {
        self.stake_token_value
    }

    pub fn unstaked_near_withdrawal_availability(&self) -> EpochHeight {
        self.stake_token_value.block_time_height().epoch_height()
            + UNSTAKED_NEAR_FUNDS_NUM_EPOCHS_TO_UNLOCK
    }

    /// returns true if unstaked funds are available to withdraw, i.e., at least 3 epochs have passed
    /// since the funds were unstaked
    pub fn unstaked_funds_available_for_withdrawal(&self) -> bool {
        self.unstaked_near_withdrawal_availability().value() <= env::epoch_height()
    }

    /// Used to track when an account has claimed their STAKE tokens for the NEAR they have staked
    pub fn stake_tokens_redeemed(&mut self, redeemed_stake: YoctoStake) {
        self.redeemed_stake -= redeemed_stake;
    }

    pub fn all_claimed(&self) -> bool {
        self.redeemed_stake.value() == 0
    }
}
