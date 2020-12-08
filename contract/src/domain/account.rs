use crate::domain::stake_batch::StakeBatch;
use crate::domain::{
    RedeemStakeBatch, StorageUsage, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct Account {
    /// account is responsible for paying for its own storage fees
    /// the funds are escrowed and refunded when the account is unregistered
    storage_escrow: TimestampedNearBalance,
    storage_usage: StorageUsage,

    /// NEAR funds that are available for withdrawal
    near: Option<TimestampedNearBalance>,
    /// STAKE tokens that the account owns
    stake: Option<TimestampedStakeBalance>,

    /// users will deposit NEAR funds into a batch that will be processed, i.e. deposited and staked
    /// into the staking pool, at scheduled intervals
    /// - STAKE token value is computed when batches are processed in order to issue STAKE tokens
    ///   for NEAR that was staked
    /// - when the account is accessed, the [StakeBatch] status is checked - if processed, then the
    ///   STAKE token value is looked up for the batch and the account is credited with STAKE tokens
    ///   and the batch is cleared
    /// - when funds are claimed, the account is refunded storage fees
    stake_batch: Option<StakeBatch>,
    /// if the contract is locked, then deposit the NEAR funds in the next batch
    next_stake_batch: Option<StakeBatch>,

    /// when a user wants to redeem STAKE tokens, they are moved from the [stake] balance into the
    /// [redeem_stake_batch] balance.
    /// - STAKE tokens become locked, i.e., they can no longer be traded
    /// - when the account is accessed, the [RedeemStakeBatch] status is checked - if processed, then
    ///   the STAKE token value is looked up for the batch and the account is credited with NEAR token
    ///   and the batch is cleared
    /// - when funds are claimed, the account is refunded storage fees
    redeem_stake_batch: Option<RedeemStakeBatch>,
    /// if the contract is locked, then deposit the NEAR funds in the next batch
    next_redeem_stake_batch: Option<RedeemStakeBatch>,
}

impl Account {
    pub fn storage_escrow(&self) -> TimestampedNearBalance {
        self.storage_escrow
    }

    pub fn apply_storage_usage_increase(
        &mut self,
        storage_usage: StorageUsage,
        storage_fee: YoctoNear,
    ) {
        if storage_usage.value() > 0 {
            assert!(
                storage_fee.value() > 0,
                "storage usage increase requires storage fee payment"
            );
            *self.storage_usage += storage_usage.value();
            self.storage_escrow.credit(storage_fee);
        }
    }

    pub fn apply_storage_usage_decrease(
        &mut self,
        storage_usage: StorageUsage,
        storage_fee: YoctoNear,
    ) {
        if storage_usage.value() > 0 {
            assert!(
                storage_fee.value() > 0,
                "storage usage decrease requires storage fee refund"
            );
            *self.storage_usage -= storage_usage.value();
            self.storage_escrow.debit(storage_fee);
        }
    }

    pub fn has_funds(&self) -> bool {
        self.near.is_some()
            || self.stake.is_some()
            || self.stake_batch.is_some()
            || self.redeem_stake_batch.is_some()
    }
}
