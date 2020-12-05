use crate::domain::timestamped_stake_balance::TimestampedStakeBalance;
use crate::{StorageUsage, TimestampedNearBalance};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Account {
    /// account is responsible for paying for its own storage fees
    /// the funds are escrowed and refunded when the account is unregistered
    storage_escrow: TimestampedNearBalance,
    storage_usage: StorageUsage,

    /// when the user is invoking `deposit_and_stake`, the funds are are first credited to the NEAR
    /// balance
    near: TimestampedNearBalance,
    /// once the NEAR funds are confirmed to be staked with the staking pool, then the staked funds
    /// are moved from the [near] balance into the [stake] balance
    stake: TimestampedStakeBalance,

    /// when a user wants to redeem STAKE tokens, they are moved from the [stake] balance into the
    /// [pending_unstake] balance.
    /// - these funds will be unstaked at the next unstaking cycle
    pending_unstake: UnstakeBatch,
    unstaked: Vec<UnstakeBatch>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct UnstakeBatch {
    batch_id: u64,
    balance: TimestampedStakeBalance,
}
