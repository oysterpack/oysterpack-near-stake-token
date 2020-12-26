use crate::domain::{RedeemStakeBatch, YoctoNear};
use crate::{
    domain::{EpochHeight, StakeTokenValue, YoctoStake},
    near::UNSTAKED_NEAR_FUNDS_NUM_EPOCHS_TO_UNLOCK,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
};

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy)]
pub struct RedeemStakeBatchReceipt {
    redeemed_stake: YoctoStake,
    stake_token_value: StakeTokenValue,
}

impl From<(RedeemStakeBatch, StakeTokenValue)> for RedeemStakeBatchReceipt {
    fn from((batch, stake_token_value): (RedeemStakeBatch, StakeTokenValue)) -> Self {
        RedeemStakeBatchReceipt::new(batch.balance().amount(), stake_token_value)
    }
}

impl RedeemStakeBatchReceipt {
    pub fn new(redeemed_stake: YoctoStake, stake_token_value: StakeTokenValue) -> Self {
        Self {
            redeemed_stake,
            stake_token_value,
        }
    }

    /// used to track claims against the receipt - as accounts claim NEAR funds for the STAKE they
    /// redeemed in the batch, the STAKE is debited
    /// - when all NEAR funds are claimed, i.e., then the receipt is deleted from storage
    pub fn redeemed_stake(&self) -> YoctoStake {
        self.redeemed_stake
    }

    /// returns the STAKE token value at the point in time when the batch was run
    pub fn stake_token_value(&self) -> StakeTokenValue {
        self.stake_token_value
    }

    /// returns the epoch within which the unstaked NEAR funds will be available for withdrawal from
    /// the staking pool
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

    /// returns true if all NEAR tokens have been claimed for the redeemed STAKE tokens, i.e., when
    /// [redeemed_stake](RedeemStakeBatchReceipt::redeemed_stake) balance is zero
    pub fn all_claimed(&self) -> bool {
        self.redeemed_stake.value() == 0
    }

    /// converts the redeemed STAKE tokens into NEAR tokens based on the receipt's [stake_token_value](RedeemStakeBatchReceipt::stake_token_value)
    pub fn stake_near_value(&self) -> YoctoNear {
        self.stake_token_value.stake_to_near(self.redeemed_stake)
    }
}
