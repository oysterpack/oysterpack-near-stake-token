use crate::domain::{StakeBatch, StakeBatchReceipt};
use crate::near::YOCTO;
use crate::{
    core::Hash,
    domain::{Account, StorageUsage, YoctoNear, YoctoNearValue},
    interface::{AccountRegistry, UnregisterAccountFailure},
    StakeTokenContract,
};
use near_sdk::{
    env,
    json_types::{ValidAccountId, U128},
    near_bindgen, Promise,
};

#[near_bindgen]
impl AccountRegistry for StakeTokenContract {
    fn account_registered(&self, account_id: ValidAccountId) -> bool {
        let hash = Hash::from(account_id.as_ref());
        self.accounts.get(&hash).is_some()
    }

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

    fn unregister_account(&mut self) -> Result<YoctoNearValue, UnregisterAccountFailure> {
        let account_id = env::predecessor_account_id();
        let account_id_hash = Hash::from(&env::predecessor_account_id());

        match self.accounts.remove(&account_id_hash) {
            None => Err(UnregisterAccountFailure::NotRegistered),
            Some(account) => {
                if account.has_funds() {
                    self.accounts.insert(&account_id_hash, &account);
                    Err(UnregisterAccountFailure::AccountHasFunds)
                } else {
                    let storage_escrow_refund = account.storage_escrow.balance();
                    Promise::new(account_id).transfer(storage_escrow_refund.value());
                    Ok(storage_escrow_refund.into())
                }
            }
        }
    }

    fn total_registered_accounts(&self) -> U128 {
        self.accounts_len.into()
    }

    /// returns the required account storage fee that needs to be attached to the account registration
    /// contract function call in yoctoNEAR
    fn account_storage_fee(&self) -> YoctoNearValue {
        let fee = self.config.storage_cost_per_byte().value()
            * self.account_storage_usage.value() as u128;
        fee.into()
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
            let mut batch = self.stake_batch.unwrap_or_else(|| {
                // create the next batch
                *self.batch_id_sequence += 1;
                StakeBatch::new(self.batch_id_sequence, YoctoNear(0))
            });
            batch.add(amount);
            self.stake_batch = Some(batch);
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

        let mut batch = account.next_stake_batch.unwrap_or_else(|| {
            account
                .stake_batch
                .unwrap_or_else(|| StakeBatch::new(self.batch_id_sequence, YoctoNear(0)))
        });

        batch.add(amount);
        if self.locked {
            account.next_stake_batch = Some(batch);
        } else {
            account.stake_batch = Some(batch);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::near::YOCTO;
    use crate::test_utils::near;
    use near_sdk::{serde_json, testing_env, AccountId, MockedBlockchain, VMContext};
    use std::convert::TryFrom;

    #[test]
    fn result_json() {
        let result: Result<YoctoNearValue, _> = Err(UnregisterAccountFailure::NotRegistered);
        let json = serde_json::to_string_pretty(&result).unwrap();
        println!(
            "Err(UnregisterAccountFailure::AlreadyRegistered) JSON: {}",
            json
        );

        let result: Result<YoctoNearValue, _> = Err(UnregisterAccountFailure::AccountHasFunds);
        let json = serde_json::to_string_pretty(&result).unwrap();
        println!(
            "Err(UnregisterAccountFailure::AlreadyRegistered) JSON: {}",
            json
        );

        let result: Result<YoctoNearValue, UnregisterAccountFailure> =
            Ok(YoctoNearValue::from(YoctoNear(YOCTO)));
        let json = serde_json::to_string_pretty(&result).unwrap();
        println!("Ok(YoctoNEAR::from(YOCTO)) JSON: {}", json);
    }

    fn operator_id() -> AccountId {
        "operator.stake.oysterpack.near".to_string()
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
    fn register_new_account_with_stake_successfully_when_contract_not_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
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
    }

    //
    // #[test]
    // fn register_preexisting_account() {
    //     let account_id = near::to_account_id("alfio-zappala.near");
    //     let mut context = near::new_context(account_id.clone());
    //     context.attached_deposit = 10 * YOCTO;
    //     testing_env!(context);
    //     let mut contract = StakeTokenContract::new(operator_id(), None);
    //
    //     match contract.register_account() {
    //         RegisterAccountResult::Registered { storage_fee } => {
    //             // when trying to register the same account again
    //             match contract.register_account() {
    //                 RegisterAccountResult::AlreadyRegistered => (), // expected
    //                 _ => panic!("expected AlreadyRegistered result"),
    //             }
    //         }
    //         RegisterAccountResult::AlreadyRegistered => {
    //             panic!("account should not be already registered");
    //         }
    //     }
    // }
    //
    // #[test]
    // #[should_panic]
    // fn register_new_account_with_no_deposit() {
    //     let account_id = near::to_account_id("alfio-zappala.near");
    //     let mut context = near::new_context(account_id.clone());
    //     context.attached_deposit = 0;
    //     testing_env!(context);
    //     let mut contract = StakeTokenContract::new(operator_id(), None);
    //     contract.register_account();
    // }
    //
    // #[test]
    // #[should_panic]
    // fn register_new_account_with_not_enough_deposit() {
    //     let account_id = near::to_account_id("alfio-zappala.near");
    //     let mut context = near::new_context(account_id.clone());
    //     context.attached_deposit = 1;
    //     testing_env!(context);
    //     let mut contract = StakeTokenContract::new(operator_id(), None);
    //     contract.register_account();
    // }
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
