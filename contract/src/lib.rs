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
    YoctoNear, YoctoNearValue,
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
    account_storage_usage: StorageUsage,

    staking_pool_id: AccountId,
    locked: bool,
}

impl StakeTokenContract {}

impl Default for StakeTokenContract {
    fn default() -> Self {
        panic!("contract should be initialized before usage")
    }
}

#[near_bindgen]
impl StakeTokenContract {
    /// ## Notes
    /// - when the contract is deployed it will measure account storage usage
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

        let mut contract = Self {
            operator_id,

            config: config.unwrap_or_else(Config::default),
            config_change_block_height: env::block_index().into(),

            accounts: Accounts::default(),
            account_storage_usage: Default::default(),
            staking_pool_id: staking_pool_id.into(),
            locked: false,
        };

        // compute account storage usage
        {
            let initial_storage_usage = env::storage_usage();
            contract
                .accounts
                .allocate_account_template_to_measure_storage_usage();
            contract.account_storage_usage =
                StorageUsage(env::storage_usage() - initial_storage_usage);
            contract
                .accounts
                .deallocate_account_template_to_measure_storage_usage();
            assert_eq!(initial_storage_usage, env::storage_usage());
        }

        contract
    }

    pub fn operator_id(&self) -> &str {
        &self.operator_id
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interface::AccountRegistry;
    use crate::near::YOCTO;
    use crate::test_utils::near;
    use crate::test_utils::near::new_context;
    use near_sdk::{testing_env, AccountId, MockedBlockchain, VMContext};
    use std::convert::TryFrom;

    #[test]
    fn state_token_contract_account_storage_usage() {
        let account_id = "bob.near";
        let context = new_context(account_id);
        testing_env!((context));

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("joe.near").unwrap();
        let contract = StakeTokenContract::new(staking_pool_id, operator_id, None);
        assert!(contract.account_storage_usage.value() > 0);
        println!(
            "account_storage_usage: {:?} -> storage fee: {} NEAR",
            contract.account_storage_usage,
            contract.account_storage_escrow_fee().value() as f64 / YOCTO as f64
        );
    }
}
