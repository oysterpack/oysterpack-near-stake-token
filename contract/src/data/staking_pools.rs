use crate::common::json_types::BlockHeight;
use crate::common::YOCTO;
use crate::data::{BlockTimestamp, TimestampedBalance};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, LookupSet, UnorderedMap};
use near_sdk::{AccountId, Balance, EpochHeight};
use primitive_types::U256;
use std::ops::{Deref, DerefMut};

pub type StakingPoolId = AccountId;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct StakingPools {
    /// aggregates deposit and stake activity across all accounts per staking pool
    deposit_and_stake_activity: UnorderedMap<StakingPoolId, TimestampedBalance /* NEAR */>,

    /// staking pools are locked when STAKE is being redeemed
    /// - while the staking pool is locked, all mutable account operations will fail on the staking pool
    locks: LookupSet<StakingPoolId>,

    /// aggregates stake token balances across all accounts per staking pool
    stake_token_balances: UnorderedMap<StakingPoolId, TimestampedBalance /* STAKE */>,

    // TODO: aggregate by batch
    /// aggregates locked stake tokens across all accounts per staking pool
    locked_stake_token_balances: UnorderedMap<StakingPoolId, TimestampedBalance /* STAKE */>,

    /// tracks unstake requests that have been submitted to staking pools
    /// - When locked STAKE tokens are redeemed, their current NEAR value is computed and recorded.
    ///   Then the amount to unstake is recorded.
    unstake_activity: UnorderedMap<StakingPoolId, StakeTokenRedemption>,

    /// queues withdrawal requests until funds are available to be withdrawn
    /// - when funds are unstaked from a staking pool, funds are not available for withdrawal until
    ///   4 epochs later
    /// - before unstaking we must ensure that all available funds are withdrawn
    pending_withdrawals: UnorderedMap<StakingPoolId, EpochHeight>,

    /// - when accounts submit requests to redeem STAKE tokens, they are grouped into batches
    /// - when the batch is processed, the batch ID is incremented
    stake_token_redemption_batch: u64,
    stake_token_redemptions:
        UnorderedMap<u64 /* batch */, UnorderedMap<StakingPoolId, StakeTokenRedemption>>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct StakeTokenRedemption {
    batch: u64,

    total_stake_supply: Balance,
    total_staked_near: Balance,

    /// timestamp info
    block_height: BlockHeight,
    block_timestamp: BlockTimestamp,
    epoch_height: EpochHeight,

    total_stake_tokens_redeemed: Balance,
    total_near_tokens_unstaked: Balance,
}

impl StakeTokenRedemption {
    /// converts yoctoSTAKE to yoctoNEAR
    ///
    /// NOTE: conversion is rounded down
    pub fn to_near(&self, stake: Balance) -> Balance {
        let value = U256::from(stake) * U256::from(self.total_staked_near)
            / U256::from(self.total_stake_supply);
        value.as_u128()
    }

    /// returns the value of 10^24 yoctoSTAKE tokens in yoctoNEAR units
    ///
    /// NOTE: conversion is rounded down
    pub fn yocto_stake_value(&self) -> Balance {
        self.to_near(YOCTO)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::near::new_context;
    use near_sdk::{testing_env, MockedBlockchain, VMContext};
}
