// TODO: remove
#![allow(unused_imports, dead_code, unused_variables)]

pub mod config;
pub mod contract;
mod core;
pub mod domain;
pub mod interface;
pub mod near;

#[cfg(test)]
pub mod test_utils;

use crate::config::Config;
use crate::core::Hash;
use crate::domain::{
    Account, Accounts, BlockHeight, StorageUsage, TimestampedNearBalance, TimestampedStakeBalance,
    YoctoNear,
};
use near_sdk::json_types::ValidAccountId;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::LookupMap,
    env, near_bindgen, wee_alloc, AccountId,
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakeTokenContract {
    /// Operator is allowed to perform operator actions on the contract
    /// TODO: support multiple operator and role management
    operator_id: AccountId,

    config: Config,
    /// when the config was last changed
    /// the block info can be looked up via its block index: https://docs.near.org/docs/api/rpc#block
    config_change_block_height: BlockHeight,

    accounts: Accounts,
    staking_pool_id: AccountId,

    locked: bool,
}

impl Default for StakeTokenContract {
    fn default() -> Self {
        panic!("contract should be initialized before usage")
    }
}

#[near_bindgen]
impl StakeTokenContract {
    #[payable]
    #[init]
    pub fn new(
        staking_pool_id: ValidAccountId,
        operator_id: ValidAccountId,
        config: Option<Config>,
    ) -> Self {
        let operator_id: AccountId = operator_id.into();
        assert_ne!(
            env::current_account_id(),
            operator_id,
            "operator account ID must not be the contract account ID"
        );

        assert!(!env::state_exists(), "contract is already initialized");

        // TODO: verify the staking pool contract interface by invoking functions that this contract depends on

        Self {
            operator_id,

            config: config.unwrap_or_else(Config::default),
            config_change_block_height: env::block_index().into(),

            accounts: Accounts::default(),
            staking_pool_id: staking_pool_id.into(),
            locked: false,
        }
    }

    pub fn operator_id(&self) -> &str {
        &self.operator_id
    }
}
