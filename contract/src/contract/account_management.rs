use crate::domain::{StakeBatch, StakeBatchReceipt};
use crate::interface::StakeAccount;
use crate::near::YOCTO;
use crate::{
    core::Hash,
    domain::{Account, StorageUsage, YoctoNear, YoctoNearValue},
    interface::{self, AccountManagement},
    StakeTokenContract,
};
use near_sdk::{
    env,
    json_types::{ValidAccountId, U128},
    near_bindgen, Promise,
};

#[near_bindgen]
impl AccountManagement for StakeTokenContract {
    /// ## Logic
    /// - check attached deposit
    ///   - assert amount is enough to cover storage fees
    /// - track the account storage fees
    /// - stake attached deposit minus account storage fees
    ///
    /// ## Panics
    /// - if attached deposit is not enough to cover account storage fees
    /// - if account is already registered
    #[payable]
    fn register_account(&mut self) {
        let account_storage_fee: YoctoNear = self.account_storage_fee().into();
        let attached_deposit = YoctoNear(env::attached_deposit());
        assert!(
            attached_deposit.value() >= account_storage_fee.value(),
            "deposit is required to pay for account storage fees : {} NEAR",
            account_storage_fee.value() as f64 / YOCTO as f64,
        );

        let mut account = Account::new(account_storage_fee);

        // stake any attached deposit minus the account storage fees
        {
            let stake_amount = attached_deposit - account_storage_fee;
            if stake_amount.value() > 0 {
                self.apply_stake_batch_credit(&mut account, stake_amount)
            }
        }

        assert!(
            self.insert_account(&Hash::from(&env::predecessor_account_id()), &account)
                .is_none(),
            "account is already registered"
        );
    }

    fn unregister_account(&mut self) {
        let account_id = env::predecessor_account_id();
        let account_id_hash = Hash::from(&env::predecessor_account_id());

        match self.remove_account(&account_id_hash) {
            None => panic!("account is not registered"),
            Some(account) => {
                assert!(
                    !account.has_funds(),
                    "all funds must be withdrawn from the account in order to unregister"
                );
                // refund the escrowed storage fee
                Promise::new(account_id).transfer(account.storage_escrow.balance().value());
            }
        };
    }

    /// returns the required account storage fee that needs to be attached to the account registration
    /// contract function call in yoctoNEAR
    fn account_storage_fee(&self) -> interface::YoctoNear {
        let fee = self.config.storage_cost_per_byte().value()
            * self.account_storage_usage.value() as u128;
        fee.into()
    }

    fn account_registered(&self, account_id: ValidAccountId) -> bool {
        let hash = Hash::from(account_id.as_ref());
        self.accounts.get(&hash).is_some()
    }

    fn total_registered_accounts(&self) -> U128 {
        self.accounts_len.into()
    }

    fn lookup_account(&self, account_id: ValidAccountId) -> Option<StakeAccount> {
        let hash = Hash::from(account_id.as_ref());
        self.accounts.get(&hash).map(Into::into)
    }

    fn withdraw(&mut self, amount: interface::YoctoNear) {
        unimplemented!()
    }

    fn withdraw_all(&mut self) {
        unimplemented!()
    }
}

impl StakeTokenContract {
    /// when a new account is registered the following is tracked:
    /// - total account count is inc
    /// - total storage escrow is updated
    fn insert_account(&mut self, account_id: &Hash, account: &Account) -> Option<Account> {
        match self.accounts.insert(account_id, account) {
            None => {
                self.accounts_len += 1;
                self.total_storage_escrow
                    .credit(account.storage_escrow.balance());
                None
            }
            Some(previous) => Some(previous),
        }
    }

    /// when a new account is registered the following is tracked:
    /// - total account count is dev
    /// - total storage escrow is updated
    fn remove_account(&mut self, account_id: &Hash) -> Option<Account> {
        match self.accounts.remove(account_id) {
            None => None,
            Some(account) => {
                self.accounts_len -= 1;
                self.total_storage_escrow
                    .debit(account.storage_escrow.balance());
                Some(account)
            }
        }
    }

    /// batches the NEAR to stake at the contract level and account level
    /// - if the account has a pre-existing batch, then check the batch's status, i.e., check if
    ///   a batch has a receipt to claim STAKE tokens
    ///   - if STAKE tokens are all claimed on the batch receipt, then delete the batch receipt
    fn apply_stake_batch_credit(&mut self, account: &mut Account, amount: YoctoNear) {
        if amount.value() == 0 {
            return;
        }

        // apply to contract level batch
        {
            if self.locked {
                let mut batch = self.next_stake_batch.unwrap_or_else(|| {
                    // create the next batch
                    *self.batch_id_sequence += 1;
                    StakeBatch::new(self.batch_id_sequence, YoctoNear(0))
                });
                batch.add(amount);
                self.next_stake_batch = Some(batch);
            } else {
                let mut batch = self.stake_batch.unwrap_or_else(|| {
                    // create the next batch
                    *self.batch_id_sequence += 1;
                    StakeBatch::new(self.batch_id_sequence, YoctoNear(0))
                });
                batch.add(amount);
                self.stake_batch = Some(batch);
            }
        }

        // check if there are STAKE tokens to claim
        {
            fn claim_stake_tokens(
                contract: &mut StakeTokenContract,
                account: &mut Account,
                batch: StakeBatch,
                receipt: &mut StakeBatchReceipt,
            ) {
                // how much NEAR did the account stake in the batch
                let staked_near = batch.balance().balance();

                // claim the STAKE tokens for the account
                let stake = receipt.stake_token_value().near_to_stake(staked_near);
                account.apply_stake_credit(stake);

                // track that the STAKE tokens were claimed
                receipt.stake_tokens_issued(staked_near);
                if receipt.all_claimed() {
                    // then delete the receipt and free the storage
                    contract.stake_batch_receipts.remove(&batch.id());
                }
            }

            if let Some(batch) = account.stake_batch {
                if let Some(mut receipt) = self.stake_batch_receipts.get(&batch.id()) {
                    claim_stake_tokens(self, account, batch, &mut receipt);
                    account.stake_batch = None;
                }
            }
            if let Some(batch) = account.next_stake_batch {
                if let Some(mut receipt) = self.stake_batch_receipts.get(&batch.id()) {
                    claim_stake_tokens(self, account, batch, &mut receipt);
                    account.next_stake_batch = None;
                }
            }
        }

        if self.locked {
            let mut batch = account
                .next_stake_batch
                .unwrap_or_else(|| StakeBatch::new(self.batch_id_sequence, YoctoNear(0)));
            batch.add(amount);
            account.next_stake_batch = Some(batch);
        } else {
            let mut batch = account
                .next_stake_batch
                .unwrap_or_else(|| StakeBatch::new(self.batch_id_sequence, YoctoNear(0)));
            batch.add(amount);
            account.stake_batch = Some(batch);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::Config;
    use crate::near::YOCTO;
    use crate::test_utils::{near, EXPECTED_ACCOUNT_STORAGE_USAGE};
    use near_sdk::{serde_json, testing_env, AccountId, MockedBlockchain, VMContext};
    use std::convert::TryFrom;

    fn operator_id() -> AccountId {
        "operator.stake.oysterpack.near".to_string()
    }

    #[test]
    fn account_registered_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        context.is_view = true;
        testing_env!(context.clone());
        assert!(!contract.account_registered(valid_account_id.clone()));
    }

    #[test]
    fn lookup_account_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        context.is_view = true;
        testing_env!(context.clone());
        assert!(contract.lookup_account(valid_account_id.clone()).is_none());
    }

    #[test]
    fn account_storage_fee_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        context.is_view = true;
        testing_env!(context.clone());
        contract.account_storage_fee();
    }

    #[test]
    fn total_registered_accounts_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        context.is_view = true;
        testing_env!(context.clone());
        contract.total_registered_accounts();
    }

    /// - Given the contract is not locked
    /// - And the account is not currently registered
    /// - When a new account is registered with attached deposit to stake
    /// - Then [AccountRegistry::account_registered()] returns true for the registered account ID
    /// - And the total accounts registered count is incremented
    /// - And the storage fee credit is applied on the account and on the contract
    /// - And the account deposit minus the storage fee is credited to the stake batch on the account
    ///   and the on the contract
    /// - And the next stake batch is set to None
    /// - And the redeem stake batches are set to None
    #[test]
    fn register_account_with_stake_when_contract_not_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        // Given the contract is not locked
        assert!(!contract.locked);
        // And the account is not currently registered
        assert!(
            !contract.account_registered(valid_account_id.clone()),
            "account should not be registered"
        );

        let storage_before_registering_account = env::storage_usage();
        contract.register_account();
        let account = contract
            .accounts
            .get(&Hash::from(account_id))
            .expect("account should be registered");
        assert!(
            contract.account_registered(valid_account_id.clone()),
            "account should be registered"
        );
        assert_eq!(
            contract.total_registered_accounts().0,
            1,
            "There should be 1 account registered"
        );

        let account_storage_usage = env::storage_usage() - storage_before_registering_account;
        assert_eq!(
            account_storage_usage, 175,
            "account storage usage changed !!! If the change is expected, then update the assert"
        );

        // And the storage fee credit is applied on the account and on the contract
        assert_eq!(
            account.storage_escrow.balance(),
            contract.account_storage_fee().into()
        );
        assert_eq!(
            contract.total_storage_escrow.balance(),
            contract.account_storage_fee().into()
        );

        // And the account deposit minus the storage fee is credited to the stake batch on the account
        // and the on the contract
        let expected_staked_near_amount =
            context.attached_deposit - contract.account_storage_fee().value();
        assert_eq!(
            account.stake_batch.unwrap().balance().balance(),
            expected_staked_near_amount.into()
        );
        assert!(account.next_stake_batch.is_none());
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        assert_eq!(
            contract.stake_batch.unwrap().balance().balance(),
            expected_staked_near_amount.into()
        );
        assert!(contract.next_stake_batch.is_none());
    }

    #[test]
    fn register_account_with_exact_storage_fee() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = EXPECTED_ACCOUNT_STORAGE_USAGE as u128
            * Config::default().storage_cost_per_byte().value();
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        assert!(contract.stake_batch.is_none());

        let account = contract
            .accounts
            .get(&Hash::from(account_id))
            .expect("account should be registered");
        assert!(account.stake_batch.is_none());
        assert!(account.next_stake_batch.is_none())
    }

    #[test]
    fn register_account_while_contract_locked_with_stake_deposit() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);
        contract.locked = true;

        contract.register_account();

        let account = contract
            .accounts
            .get(&Hash::from(account_id))
            .expect("account should be registered");
        let expected_staked_near_amount =
            context.attached_deposit - contract.account_storage_fee().value();
        assert!(account.stake_batch.is_none());
        assert_eq!(
            account
                .next_stake_batch
                .unwrap()
                .balance()
                .balance()
                .value(),
            expected_staked_near_amount
        );

        assert!(contract.stake_batch.is_none());
        assert_eq!(
            contract
                .next_stake_batch
                .unwrap()
                .balance()
                .balance()
                .value(),
            expected_staked_near_amount
        );
    }

    #[test]
    #[should_panic(expected = "account is already registered")]
    fn register_preexisting_account() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
        contract.register_account();
    }

    #[test]
    #[should_panic(expected = "deposit is required to pay for account storage fees")]
    fn register_account_with_no_attached_deposit() {
        let account_id = "alfio-zappala.near";
        let context = near::new_context(account_id);
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
    }

    #[test]
    #[should_panic(expected = "deposit is required to pay for account storage fees")]
    fn register_account_with_insufficient_deposit_for_storage_fees() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 1;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
    }

    #[test]
    fn lookup_account() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        assert!(contract.lookup_account(valid_account_id.clone()).is_none());
        contract.register_account();

        let stake_account = contract.lookup_account(valid_account_id.clone()).unwrap();
        let stake_account_json = serde_json::to_string_pretty(&stake_account).unwrap();
        println!("{}", stake_account_json);
    }

    #[test]
    fn unregister_registered_account_with_no_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = EXPECTED_ACCOUNT_STORAGE_USAGE as u128
            * Config::default().storage_cost_per_byte().value();
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        assert!(contract.lookup_account(valid_account_id.clone()).is_none());
        contract.register_account();
        assert!(contract.account_registered(valid_account_id.clone()));
        let stake_account = contract.lookup_account(valid_account_id.clone()).unwrap();
        assert!(stake_account.stake_batch.is_none());
        assert!(stake_account.next_stake_batch.is_none());
        assert!(stake_account.near.is_none());
        assert!(stake_account.stake.is_none());

        let contract_balance_with_registered_account = env::account_balance();
        assert_eq!(
            contract.total_storage_escrow.balance().value(),
            contract_balance_with_registered_account
        );
        contract.unregister_account();
        assert!(!contract.account_registered(valid_account_id.clone()));
        assert_eq!(
            contract.total_storage_escrow.balance().value(),
            0,
            "storage fees should have been refunded"
        );
        assert_eq!(
            env::account_balance(),
            0,
            "storage fees should have been refunded"
        );
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn unregister_account_with_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = EXPECTED_ACCOUNT_STORAGE_USAGE as u128
            * Config::default().storage_cost_per_byte().value()
            + 1;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
        contract.unregister_account();
    }

    #[test]
    #[should_panic(expected = "account is not registered")]
    fn unregister_unknown_account() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = EXPECTED_ACCOUNT_STORAGE_USAGE as u128
            * Config::default().storage_cost_per_byte().value()
            + 1;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.unregister_account();
    }

    //
    // #[test]
    // fn unregister_account_with_zero_funds() {
    //     let account_id = near::to_account_id("alfio-zappala.near");
    //     let mut context = near::new_context(account_id.clone());
    //     context.attached_deposit = 10 * YOCTO;
    //     testing_env!(context);
    //     let mut contract = StakeTokenContract::new(operator_id(), None);
    //     match contract.register_account() {
    //         RegisterAccountResult::Registered { storage_fee } => {
    //             match contract.unregister_account() {
    //                 UnregisterAccountResult::Unregistered { storage_fee_refund } => {
    //                     assert_eq!(storage_fee.0, storage_fee_refund.0);
    //                     assert_eq!(contract.registered_accounts_count().0, 0);
    //                 }
    //                 result => panic!("unexpected result: {:?}", result),
    //             }
    //         }
    //         _ => panic!("registration failed"),
    //     }
    // }
    //
    // #[test]
    // fn unregister_non_existent_account() {
    //     let account_id = near::to_account_id("alfio-zappala.near");
    //     let mut context = near::new_context(account_id.clone());
    //     context.attached_deposit = 10 * YOCTO;
    //     testing_env!(context);
    //     let mut contract = StakeTokenContract::new(operator_id(), None);
    //     match contract.unregister_account() {
    //         UnregisterAccountResult::NotRegistered => (), // expected
    //         result => panic!("unexpected result: {:?}", result),
    //     }
    // }
    //
    // #[test]
    // fn unregister_account_with_near_funds() {
    //     let account_id = near::to_account_id("alfio-zappala.near");
    //     let mut context = near::new_context(account_id.clone());
    //     context.attached_deposit = 10 * YOCTO;
    //     testing_env!(context);
    //     let mut contract = StakeTokenContract::new(operator_id(), None);
    //     match contract.register_account() {
    //         RegisterAccountResult::Registered { storage_fee } => {
    //             let account_hash = Hash::from(account_id.as_bytes());
    //             let mut account = contract.accounts.get(&account_id).unwrap();
    //             account.apply_near_credit(10);
    //             contract.accounts.insert(&account_id, &account);
    //             match contract.unregister_account() {
    //                 UnregisterAccountResult::AccountHasFunds => (), // expected
    //                 result => panic!("unexpected result: {:?}", result),
    //             }
    //         }
    //         _ => panic!("registration failed"),
    //     }
    // }
    //
    // #[test]
    // fn unregister_account_with_stake_funds() {
    //     let account_id = near::to_account_id("alfio-zappala.near");
    //     let mut context = near::new_context(account_id.clone());
    //     context.attached_deposit = 10 * YOCTO;
    //     testing_env!(context);
    //     let mut contract = StakeTokenContract::new(operator_id(), None);
    //     match contract.register_account() {
    //         RegisterAccountResult::Registered { storage_fee } => {
    //             let account_hash = Hash::from(account_id.as_bytes());
    //             let mut account: Account = contract.accounts.get(&account_id).unwrap();
    //             account.apply_deposit_and_stake_activity(&account_id, 10);
    //             contract.accounts.insert(&account_id, &account);
    //             match contract.unregister_account() {
    //                 UnregisterAccountResult::AccountHasFunds => (), // expected
    //                 result => panic!("unexpected result: {:?}", result),
    //             }
    //         }
    //         _ => panic!("registration failed"),
    //     }
    // }
}
