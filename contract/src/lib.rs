// TODO: remove
#![allow(unused_imports, dead_code, unused_variables)]

pub mod account;
pub mod common;
pub mod config;
pub mod data;
pub mod stake;
pub mod staking;

#[cfg(test)]
pub mod test_utils;

use crate::common::json_types;
use crate::config::Config;
use crate::data::accounts::*;
use crate::data::staking_pools::StakingPoolId;
use crate::data::{TimestampedBalance, DEPOSIT_AND_STAKE_ACTIVITY_KEY_PREFIX};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{UnorderedMap, UnorderedSet},
    env,
    json_types::{ValidAccountId, U64},
    near_bindgen, wee_alloc, AccountId, Balance, BlockHeight, StorageUsage,
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakeTokenService {
    /// Operator is allowed to perform operator actions on the contract
    /// TODO: support multiple operator and role management
    operator_id: AccountId,

    config: Config,
    /// TODO: should the block timestamp be recorded as well?
    /// when the config was last changed
    /// the block info can be looked up via its block index: https://docs.near.org/docs/api/rpc#block
    config_change_block_height: BlockHeight,

    accounts: Accounts,
    /// used to track deposit and stake activity
    /// - when a deposit_and_stake request is submitted to the staking pool, the balance is credited
    /// - when the deposit_and_stake activity is complete (success or failure), then the balance is debited
    /// - thus, if an entry is present then it means there are pending deposit_and_stake requests
    /// TODO: track activity count
    deposit_and_stake_activity: UnorderedMap<StakingPoolId, TimestampedBalance>,
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

        assert!(!env::state_exists(), "contract is already initialized");
        Self {
            operator_id: check_operator_id(operator_id),
            config: config.unwrap_or_else(Config::default),
            config_change_block_height: env::block_index(),
            accounts: Accounts::default(),
            deposit_and_stake_activity: UnorderedMap::new(
                DEPOSIT_AND_STAKE_ACTIVITY_KEY_PREFIX.to_vec(),
            ),
        }
    }

    pub fn operator_id(&self) -> &str {
        &self.operator_id
    }

    pub fn config_change_block_height(&self) -> json_types::BlockHeight {
        self.config_change_block_height.into()
    }
}

impl StakeTokenService {
    /// asserts that the predecessor account ID must be the operator
    pub(crate) fn assert_is_operator(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.operator_id,
            "function can only be invoked by the operator"
        );
    }

    /// Computes if storage fees need to be applied and asserts that enough deposit was attached
    /// to pay for storage fees.
    ///
    /// # Panics
    /// if not enough deposit was attached to pay for account storage
    pub(crate) fn assert_storage_fees(&self, initial_storage: StorageUsage) -> Balance {
        let current_storage = env::storage_usage();
        let attached_deposit = env::attached_deposit();
        let required_deposit =
            Balance::from(current_storage - initial_storage) * self.config.storage_cost_per_byte();
        assert!(
            required_deposit <= attached_deposit,
            "The attached deposit ({}) is not enough {} to pay account storage fees: {}",
            attached_deposit,
            required_deposit - attached_deposit,
            required_deposit,
        );
        required_deposit
    }
}

#[cfg(test)]
mod test {
    use crate::test_utils::near;
    use near_sdk::json_types::U128;
    use near_sdk::{testing_env, MockedBlockchain, VMContext};

    use super::*;
    use crate::config::GasConfig;

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
        assert_eq!(contract.config_change_block_height().0, env::block_index());
    }

    #[test]
    fn contract_init_with_config() {
        let context = near::new_context(near::stake_contract_account_id());
        testing_env!(context);
        let config = Config::new(100, GasConfig::default());
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
            let contract =
                StakeTokenService::new(near::to_account_id("operator.stake.oysterpack.near"), None);
            // the NEAR runtime will persist the contract state to storage once init returns
            // however in the mocked environment it does not, thus we are manually simulating this NEAR
            // runtime behavior
            env::state_write(&contract);
        }
    }
}
