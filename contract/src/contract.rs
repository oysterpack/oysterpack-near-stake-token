pub mod account_registry;

use crate::domain::Accounts;
use crate::{
    config::Config,
    core::Hash,
    domain::{Account, StorageUsage, YoctoNear},
    near::storage_keys::ACCOUNTS_KEY_PREFIX,
    StakeTokenContract,
};
use near_sdk::{collections::LookupMap, env, json_types::ValidAccountId, near_bindgen, AccountId};

impl StakeTokenContract {
    /// asserts that the predecessor account ID must be the operator
    fn assert_is_operator(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.operator_id,
            "function can only be invoked by the operator"
        );
    }

    /// Computes if storage fees need to be applied and asserts that enough deposit was attached
    /// to pay for storage fees.
    ///
    /// Returns storage use increase and fee
    ///
    /// # Panics
    /// if not enough deposit was attached to pay for account storage
    fn assert_storage_fees(
        &self,
        initial_storage: StorageUsage,
    ) -> Option<(StorageUsage, YoctoNear)> {
        let current_storage = env::storage_usage();
        if current_storage < *initial_storage {
            return None;
        }
        let attached_deposit = env::attached_deposit();
        let storage_usage_increase = current_storage - *initial_storage;

        let required_deposit: u128 =
            (storage_usage_increase as u128) * self.config.storage_cost_per_byte().value();
        assert!(
            required_deposit <= attached_deposit,
            "The attached deposit ({}) is not enough {} to pay account storage fees: {}",
            attached_deposit,
            required_deposit - attached_deposit,
            required_deposit,
        );
        Some((storage_usage_increase.into(), required_deposit.into()))
    }

    /// Returns registered account for predecessor account.
    ///
    /// ## Panics
    /// if the predecessor account is not registered
    fn expect_registered_predecessor_account(&self) -> Account {
        let account_id_hash = Hash::from(&env::predecessor_account_id());
        self.accounts
            .get(&account_id_hash)
            .expect("account is not registered")
    }
}

#[cfg(test)]
mod test {
    // use crate::test_utils::near;

    // use near_sdk::{testing_env, MockedBlockchain, VMContext};

    // use super::*;

    // #[test]
    // fn contract_init_with_default_config() {
    //     let mut context = near::new_context(near::stake_contract_account_id());
    //     context.block_index = 10;
    //     testing_env!(context);
    //     let contract =
    //         StakeTokenContract::new(near::to_account_id("operator.stake.oysterpack.near"), None);
    //     assert_eq!(
    //         contract.config.storage_cost_per_byte(),
    //         100_000_000_000_000_000_000
    //     );
    //     assert_eq!(env::block_index(), 10);
    //     assert_eq!(contract.config_change_block_height().0, env::block_index());
    // }
    //
    // #[test]
    // fn contract_init_with_config() {
    //     let context = near::new_context(near::stake_contract_account_id());
    //     testing_env!(context);
    //     let config = updates::Config {
    //         gas_config: Some(updates::GasConfig::default()),
    //         storage_cost_per_byte: Some("100".to_string()),
    //     };
    //     let contract = StakeTokenContract::new(
    //         near::to_account_id("operator.stake.oysterpack.near"),
    //         Some(config),
    //     );
    //     assert_eq!(contract.config.storage_cost_per_byte(), 100);
    // }
    //
    // #[test]
    // #[should_panic]
    // fn contract_init_operator_id_must_not_be_contract_account() {
    //     let context = near::new_context(near::stake_contract_account_id());
    //     testing_env!(context);
    //     let contract = StakeTokenContract::new(near::stake_contract_account_id(), None);
    // }
    //
    // #[test]
    // #[should_panic]
    // fn contract_init_with_invalid_operator_id() {
    //     let context = near::new_context(near::stake_contract_account_id());
    //     testing_env!(context);
    //     let contract = StakeTokenContract::new(near::to_account_id("invalid***"), None);
    // }
    //
    // #[test]
    // #[should_panic]
    // fn contract_init_with_empty_operator_id() {
    //     let context = near::new_context(near::stake_contract_account_id());
    //     testing_env!(context);
    //     let contract = StakeTokenContract::new(near::to_account_id(""), None);
    // }
    //
    // #[test]
    // #[should_panic]
    // fn contract_init_with_blank_operator_id() {
    //     let context = near::new_context(near::stake_contract_account_id());
    //     testing_env!(context);
    //     let contract = StakeTokenContract::new(near::to_account_id("   "), None);
    // }
    //
    // #[test]
    // #[should_panic]
    // fn contract_init_will_panic_if_called_more_than_once() {
    //     let context = near::new_context(near::stake_contract_account_id());
    //     testing_env!(context);
    //     for _ in 0..2 {
    //         let contract = StakeTokenContract::new(
    //             near::to_account_id("operator.stake.oysterpack.near"),
    //             None,
    //         );
    //         // the NEAR runtime will persist the contract state to storage once init returns
    //         // however in the mocked environment it does not, thus we are manually simulating this NEAR
    //         // runtime behavior
    //         env::state_write(&contract);
    //     }
    // }
}
