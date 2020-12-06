use crate::domain::{
    RedeemStakeBatch, StorageUsage, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct StakingPool {
    staked_balance: TimestampedNearBalance,
}
