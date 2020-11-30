// TODO: remove
#![allow(unused_imports, dead_code, unused_variables)]

pub mod account;
pub mod common;
pub mod config;
pub mod events;
pub mod stake;
pub mod state;

#[cfg(test)]
pub mod test_utils;

use crate::account::Accounts;
use crate::config::Config;
use near_sdk::json_types::{ValidAccountId, U64};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, wee_alloc, AccountId, BlockHeight,
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakeTokenService {
    /// Operator is allowed to perform operator actions on the contract
    operator_id: AccountId,
    config: Config,
    config_updated_on: BlockHeight,

    accounts: Accounts,
}

impl Default for StakeTokenService {
    fn default() -> Self {
        panic!("contract should be initialized before usage")
    }
}

#[near_bindgen]
impl StakeTokenService {
    #[init]
    pub fn new(operator_id: AccountId, config: Option<Config>) -> Self {
        assert!(!env::state_exists(), "contract is already initialized");
        let contract = Self {
            operator_id: StakeTokenService::check_operator_id(operator_id),
            config: config.unwrap_or_else(Config::default),
            config_updated_on: env::block_index(),
            accounts: Accounts::default(),
        };
        env::state_write(&contract);
        contract
    }

    pub fn operator_id(&self) -> &str {
        &self.operator_id
    }

    pub fn config_updated_on_block(&self) -> U64 {
        self.config_updated_on.into()
    }
}

impl StakeTokenService {
    fn check_operator_id(operator_id: AccountId) -> AccountId {
        assert!(
            env::is_valid_account_id(operator_id.as_bytes()),
            "operator ID is not a valid AccountID: {}",
            operator_id
        );
        assert_ne!(
            env::current_account_id(),
            operator_id,
            "operator account ID must not be the contract account ID"
        );
        operator_id
    }

    /// asserts that the predecessor account ID must be the operator
    fn assert_is_operator(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.operator_id,
            "function can only be invoked by the operator"
        );
    }
}

#[cfg(test)]
mod test {
    use crate::test_utils::near;
    use near_sdk::json_types::U128;
    use near_sdk::{testing_env, MockedBlockchain, VMContext};

    use super::*;

    #[test]
    fn contract_init_with_default_config() {
        let mut context = near::new_context(near::stake_contract_account_id());
        context.block_index = 10;
        testing_env!(context);
        let contract =
            StakeTokenService::new(near::to_account_id("operator.stake.oysterpack.near"), None);
        assert_eq!(
            contract.config.storage_cost_per_byte(),
            100_000_000_000_000_000_000
        );
        assert_eq!(env::block_index(), 10);
        assert_eq!(contract.config_updated_on_block().0, env::block_index());
    }

    #[test]
    fn contract_init_with_config() {
        let context = near::new_context(near::stake_contract_account_id());
        testing_env!(context);
        let config = Config::new(100);
        let contract = StakeTokenService::new(
            near::to_account_id("operator.stake.oysterpack.near"),
            Some(config),
        );
        assert_eq!(contract.config.storage_cost_per_byte(), 100);
    }

    #[test]
    #[should_panic]
    fn contract_init_operator_id_must_not_be_contract_account() {
        let context = near::new_context(near::stake_contract_account_id());
        testing_env!(context);
        let contract = StakeTokenService::new(near::stake_contract_account_id(), None);
    }

    #[test]
    #[should_panic]
    fn contract_init_with_invalid_operator_id() {
        let context = near::new_context(near::stake_contract_account_id());
        testing_env!(context);
        let contract = StakeTokenService::new(near::to_account_id("invalid***"), None);
    }

    #[test]
    #[should_panic]
    fn contract_init_with_empty_operator_id() {
        let context = near::new_context(near::stake_contract_account_id());
        testing_env!(context);
        let contract = StakeTokenService::new(near::to_account_id(""), None);
    }

    #[test]
    #[should_panic]
    fn contract_init_with_blank_operator_id() {
        let context = near::new_context(near::stake_contract_account_id());
        testing_env!(context);
        let contract = StakeTokenService::new(near::to_account_id("   "), None);
    }

    #[test]
    #[should_panic]
    fn contract_init_will_panic_if_called_more_than_once() {
        let context = near::new_context(near::stake_contract_account_id());
        testing_env!(context);
        for _ in 0..2 {
            StakeTokenService::new(near::to_account_id("operator.stake.oysterpack.near"), None);
        }
    }
}
