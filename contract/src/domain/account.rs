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

    /// when the user is invoking `deposit_and_stake`, the funds are are first credited to the NEAR
    /// balance
    near: Option<TimestampedNearBalance>,
    /// once the NEAR funds are confirmed to be staked with the staking pool, then the staked funds
    /// are moved from the [near] balance into the [stake] balance
    stake: Option<TimestampedStakeBalance>,

    /// when a user wants to redeem STAKE tokens, they are moved from the [stake] balance into the
    /// [redeem_stake_batch] balance.
    redeem_stake_batch: Option<RedeemStakeBatch>,
    pending_withdrawal: Option<Vec<RedeemStakeBatch>>,
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
            self.storage_usage += storage_usage;
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
            self.storage_usage -= storage_usage;
            self.storage_escrow.debit(storage_fee);
        }
    }

    pub fn has_funds(&self) -> bool {
        self.near.is_some()
            || self.stake.is_some()
            || self.redeem_stake_batch.is_some()
            || self.pending_withdrawal.is_some()
    }
}
