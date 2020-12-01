use crate::common::{
    assert_self,
    json_types::{self, YoctoNEAR, YoctoSTAKE},
    StakingPoolId, YOCTO, ZERO_BALANCE,
};
use crate::StakeTokenService;
use near_sdk::{
    env, ext_contract,
    json_types::{U128, U64},
    near_bindgen,
    serde::{self, Deserialize, Serialize},
    AccountId, Balance, Promise,
};
use primitive_types::U256;
use std::collections::HashMap;

pub trait StakingService {
    /// Deposits the attached amount into the predecessor account and stakes it with the specified
    /// staking pool contract. Once the funds are successfully staked, then STAKE tokens are issued
    /// to the predecessor account.
    ///
    /// Promise returns [StakeReceipt]
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

    /// Promise returns `Option<StakeTokenValue>`
    fn stake_token_value(&self, staking_pool_id: StakingPoolId) -> Promise;
}

/// Returns the STAKE token value at the specified block height.
/// STAKE token value is computed as [total_staked] / [total_supply]
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeTokenValue {
    pub token_supply: YoctoSTAKE,
    pub staked_balance: YoctoNEAR,
    pub block_height: json_types::BlockHeight,
    pub block_timestamp: json_types::BlockTimestamp,
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
    pub supply: YoctoSTAKE,

    /// when the supply last changed
    pub block_timestamp: json_types::BlockTimestamp,
    pub block_height: json_types::BlockHeight,
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

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn get_account_staked_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn get_account_unstaked_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn get_account_total_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn deposit_and_stake(&mut self);

    fn withdraw_all(&mut self);

    fn unstake(&mut self, amount: json_types::Balance);
}

#[ext_contract(ext_staking_pool_callbacks)]
pub trait ExtStakingPoolCallbacks {
    fn on_get_account_staked_balance(
        &self,
        #[callback] balance: json_types::Balance,
        staking_pool_id: StakingPoolId,
    ) -> Option<StakeTokenValue>;

    fn on_get_account_unstaked_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn on_get_account_total_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn on_deposit_and_stake(&mut self, account_id: AccountId, stake_deposit: Balance);

    fn on_withdraw_all(&mut self);

    fn on_unstake(&mut self, amount: json_types::Balance);
}

#[near_bindgen]
impl StakingService for StakeTokenService {
    #[payable]
    fn deposit_and_stake(&mut self, staking_pool_id: StakingPoolId) -> Promise {
        let mut account = self.expect_registered_predecessor_account();
        assert!(env::attached_deposit() > 0, "no deposit was attached");

        // TODO: how expensive is this in terms of gas? If this is expensive, then we can optimize
        // by first checking if there is a STAKE balance for the staking pool
        let initial_storage_usage = env::storage_usage();

        // if the account is not currently staking with the staking pool, then account storage
        // will need to be allocated to track the staking pool
        let storage_fee = if account.init_staking_pool(&staking_pool_id) {
            self.assert_storage_fees(initial_storage_usage)
        } else {
            ZERO_BALANCE
        };

        let stake_deposit = env::attached_deposit() - storage_fee;
        assert!(
            stake_deposit > 0,
            "After applying storage fees ({} yoctoNEAR) there was zero deposit to stake."
        );

        unimplemented!()
    }

    fn staking_pool_count(&self) -> u32 {
        unimplemented!()
    }

    fn staking_pool_ids(&self, start_index: u32, max_results: u32) -> StakingPoolIDs {
        unimplemented!()
    }

    fn stake_token_supply(&self, staking_pool: StakingPoolId) -> StakeSupply {
        unimplemented!()
    }

    fn stake_token_supply_batch(
        &self,
        staking_pools: Vec<StakingPoolId>,
    ) -> HashMap<StakingPoolId, StakeSupply> {
        unimplemented!()
    }

    fn stake_token_value(&self, staking_pool_id: StakingPoolId) -> Promise {
        unimplemented!()
    }
}

/// NEAR contract callbacks are all private, they should only be invoked by itself
#[near_bindgen]
impl StakeTokenService {
    pub fn on_get_account_staked_balance(
        &self,
        #[callback] balance: json_types::Balance,
        staking_pool_id: StakingPoolId,
    ) -> Option<StakeTokenValue> {
        assert_self();

        if balance.0 == 0 {
            // there is no NEAR staked with this staking pool
            return None;
        }

        let value = StakeTokenValue {
            staked_balance: balance.into(),
            token_supply: self.stake_supply.get(&staking_pool_id).unwrap_or(0).into(),
            block_height: env::block_index().into(),
            block_timestamp: env::block_timestamp().into(),
        };
        Some(value)
    }
}
