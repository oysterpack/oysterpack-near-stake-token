use crate::domain::{
    RedeemStakeBatch, StorageUsage, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::AccountId;

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct StakingPool {
    /// staking pool account ID
    account_id: AccountId,

    /// the pool is locked in order to redeem STAKE tokens, i.e., unstake from the staking pool
    /// - the pool is locked to freeze balances in order to compute the STAKE token value in NEAR
    /// - while locked, contract function calls that would change balances are not allowed:
    ///   - deposit_and_stake
    ///   - redeem
    /// - STAKE token transfers are still allowed
    locked: bool,
}
