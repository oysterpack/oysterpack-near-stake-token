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

    fn register_account(&mut self) {
        let deposit = YoctoNear(env::attached_deposit());
        assert!(
            deposit.value() >= self.account_storage_escrow_fee().value(),
            "deposit is required to pay for account storage fees : {} NEAR",
            self.account_storage_escrow_fee().value() as f64 / YOCTO as f64,
        );

        let account = Account::new(self.account_storage_escrow_fee().value().into());
        let account_id_hash = Hash::from(&env::predecessor_account_id());
        assert!(
            self.accounts.insert(&account_id_hash, &account).is_none(),
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
                    let storage_escrow_refund = account.storage_escrow().balance();
                    Promise::new(account_id).transfer(storage_escrow_refund.value());
                    Ok(storage_escrow_refund.into())
                }
            }
        }
    }

    fn registered_accounts_count(&self) -> U128 {
        self.accounts.count().into()
    }

    /// returns the required account storage fee that needs to be attached to the account registration
    /// contract function call in yoctoNEAR
    fn account_storage_escrow_fee(&self) -> YoctoNearValue {
        let fee = self.config.storage_cost_per_byte().value()
            * self.account_storage_usage.value() as u128;
        fee.into()
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

    #[test]
    fn register_new_account_success() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context);

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);
        assert!(
            !contract.account_registered(valid_account_id.clone()),
            "account should not be registered"
        );
        assert_eq!(
            contract.registered_accounts_count().0,
            0,
            "There should be no accounts registered"
        );

        let storage_before_registering_account = env::storage_usage();
        contract.register_account();
        let account_storage_usage = env::storage_usage() - storage_before_registering_account;
        assert_eq!(
            account_storage_usage, 119,
            "account storage usage changed !!! If the change is expected, then update the assert"
        );

        assert!(
            contract.account_registered(valid_account_id.clone()),
            "account should be registered"
        );
        assert_eq!(
            contract.registered_accounts_count().0,
            1,
            "There should be 1 account registered"
        );
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
