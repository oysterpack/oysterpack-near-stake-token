use crate::common::{
    json_types::{self, YoctoNEAR, YoctoSTAKE},
    StakingPoolId, YOCTO,
};
use near_sdk::json_types::{U128, U64};
use near_sdk::{
    serde::{self, Deserialize, Serialize},
    AccountId, Promise,
};
use primitive_types::U256;
use std::collections::HashMap;

pub trait StakingService {
    /// Deposits the attached amount into the predecessor account and stakes it with the specified
    /// staking pool contract. Once the funds are successfully staked, then STAKE tokens are issued
    /// to the predecessor account.
    ///
    /// ## Account Storage Fees
    /// - any applicable storage fees will be deducted from the attached deposit
    /// - when staking to a new staking pool, then storage fees will be charged
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if not enough deposit was attached to cover storage fees
    fn deposit_and_stake(&mut self, staking_pool_id: StakingPoolId) -> Promise;

    /// Returns the number of staking pools that are staked with across all accounts.
    ///
    /// NOTE: there are STAKE tokens issued per staking pool
    fn staking_pool_count(&self) -> u32;

    /// Enables paging through the staking pool account IDs.
    /// - [start_index] defines the starting point for the iterator
    ///   - if the [start_index] is out of range, then an empty result set is returned, i.e., empty Vec
    /// - [max_results] defines the maximum number of results to return, i.e., page size
    fn staking_pool_ids(&self, start_index: u32, max_results: u32) -> StakingPoolIDs;

    /// Returns the number of STAKE tokens issued for the specified staking pool.
    fn stake_token_supply(&self, staking_pool: StakingPoolId) -> StakeSupply;

    /// Retrieves STAKE token supplies for a batch of staking pools
    fn stake_token_supply_batch(
        &self,
        staking_pools: Vec<StakingPoolId>,
    ) -> HashMap<StakingPoolId, StakeSupply>;

    fn stake_token_value(&self, staking_pool_id: StakingPoolId) -> Option<StakeTokenValue>;
}

/// Returns the STAKE token value at the specified block height.
/// STAKE token value is computed as [total_staked] / [total_supply]
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeTokenValue {
    pub token_supply: YoctoSTAKE,
    pub staked_balance: YoctoNEAR,
    pub block_height: json_types::BlockHeight,
}

impl StakeTokenValue {
    pub fn value(&self) -> YoctoNEAR {
        if self.staked_balance.0 == 0 || self.token_supply.0 == 0 {
            return YOCTO.into();
        }
        let value =
            U256::from(YOCTO) * U256::from(self.staked_balance.0) / U256::from(self.token_supply.0);
        value.as_u128().into()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolIDs {
    pub staking_pool_ids: Vec<StakingPoolId>,
    pub start_index: u32,
    pub staking_pool_total_count: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeSupply {
    pub supply: U128,

    /// when the supply last changed
    pub block_timestamp: U64,
    pub block_height: U64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeReceipt {
    /// amount in yoctoNEAR that was staked
    pub near_tokens_staked: YoctoNEAR,
    /// amount of yoctoSTAKE that was credited to the customer account
    pub stake_tokens_credit: YoctoSTAKE,
    /// amount of storage fees that were deducted
    ///
    /// ## Notes
    /// - storage fees will be charged when staking with a new staking pool
    pub storage_fees: Option<YoctoNEAR>,
}
