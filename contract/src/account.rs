use crate::common::{Hash, StakingPoolAccountId, YoctoNEAR};
use crate::stake::YoctoSTAKE;
use crate::StakeTokenService;
use near_sdk::json_types::U128;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, UnorderedMap},
    env, near_bindgen,
    serde::{self, Deserialize, Serialize},
    AccountId, Balance, BlockHeight, EpochHeight, Promise, StorageUsage,
};
use primitive_types::U256;
use std::collections::HashMap;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Accounts {
    /// when a new account is registered it is assigned the next sequence value
    accounts: LookupMap<Hash, Account>,
    /// using u128 to make this future proof ... at least for the forseeable future
    /// - use case: IOT, e.g. every device could potentially have its own account
    count: u128,
}

impl Accounts {
    pub fn remove_account(&mut self, account_id: AccountId) -> Option<Account> {
        let account_hash = Hash::from(account_id.as_str());
        match self.accounts.remove(&account_hash) {
            None => None,
            Some(account) => {
                self.count -= 1;
                Some(account)
            }
        }
    }
}

impl Default for Accounts {
    fn default() -> Self {
        Self {
            count: 0,
            accounts: LookupMap::new(b"b".to_vec()),
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Account {
    storage_escrow: Balance,
    /// STAKE token balances per staking pool
    stake_balances: UnorderedMap<StakingPoolAccountId, Balance>,
    available_near_balance: Balance,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            storage_escrow: 0,
            available_near_balance: 0,
            stake_balances: UnorderedMap::new(b"c".to_vec()),
        }
    }
}

trait AccountRegistry {
    fn account_registered(&self, account_id: AccountId) -> bool;

    /// If no account exists for the predecessor account ID, then a new one is created and registered.
    /// The account is required to pay for its storage. Storage fees will be escrowed and refunded
    /// when the account is unregistered.
    ///
    /// Returns false if the account is already registered.
    /// If the account is already registered, then the deposit is refunded.
    ///
    /// #[payable]
    /// - account must pay for its storage
    /// - storage fee: ??? yoctoNEAR
    ///
    /// ## Panics
    /// if deposit is not enough to cover storage fees
    fn register_account(&mut self) -> RegisterAccountResult;

    /// An account can only be unregistered if the account has zero token balance, i.e., zero STAKE
    /// and NEAR balances. In order to unregister the account all NEAR must be unstaked and withdrawn
    /// from the account.
    fn unregister_account(&mut self) -> UnregisterAccountResult;

    fn registered_accounts_count(&self) -> U128;
}

#[near_bindgen]
impl AccountRegistry for StakeTokenService {
    fn account_registered(&self, account_id: AccountId) -> bool {
        self.accounts
            .accounts
            .contains_key(&Hash::from(account_id.as_str()))
    }

    #[payable]
    fn register_account(&mut self) -> RegisterAccountResult {
        fn check_args() -> (AccountId, Balance) {
            let deposit = env::attached_deposit();
            assert!(
                deposit > 0,
                "deposit is required to pay for account storage fees",
            );

            let account_id = env::predecessor_account_id();
            assert_ne!(account_id, env::current_account_id());

            (account_id, deposit)
        }

        fn apply_storage_fees(
            contract: &StakeTokenService,
            initial_storage: StorageUsage,
        ) -> Balance {
            let current_storage = env::storage_usage();
            let attached_deposit = env::attached_deposit();
            let required_deposit = Balance::from(current_storage - initial_storage)
                * contract.config.storage_cost_per_byte();
            assert!(
                required_deposit <= attached_deposit,
                "The attached deposit ({}) is short {} to cover account storage fees: {}",
                attached_deposit,
                required_deposit - attached_deposit,
                required_deposit,
            );
            let refund_amount = attached_deposit - required_deposit;
            env::log(format!("Storage fee refund: {}", refund_amount).as_bytes());
            Promise::new(env::predecessor_account_id()).transfer(refund_amount);
            required_deposit
        }

        let (account_id, deposit) = check_args();

        let account_hash = Hash::from(account_id.as_str());
        if self.accounts.accounts.contains_key(&account_hash) {
            return RegisterAccountResult::AlreadyRegistered;
        }

        // account needs to pay for its storage
        // the amount of storage will be determined dynamically
        let initial_storage_usage = env::storage_usage();
        let mut account = Account::default();
        self.accounts.accounts.insert(&account_hash, &account);
        // this has the potential to overflow in the far distant future ...
        self.accounts.count += 1;

        let storage_fee = apply_storage_fees(self, initial_storage_usage);
        account.storage_escrow = storage_fee;
        self.accounts.accounts.insert(&account_hash, &account);
        RegisterAccountResult::Registered {
            storage_fee: storage_fee.into(),
        }
    }

    fn unregister_account(&mut self) -> UnregisterAccountResult {
        let account_id = env::predecessor_account_id();
        let account_hash = Hash::from(account_id.as_str());
        match self.accounts.accounts.get(&account_hash) {
            None => UnregisterAccountResult::NotRegistered,
            Some(account) => {
                if account.available_near_balance > 0 || !account.stake_balances.is_empty() {
                    UnregisterAccountResult::AccountHasFunds
                } else {
                    // TODO: Is it safe to transfer async?
                    // What happens to the funds if the transfer fails?
                    // - Are the funds refunded back to this contract?
                    Promise::new(account_id).transfer(account.storage_escrow);
                    self.accounts.accounts.remove(&account_hash);
                    UnregisterAccountResult::Unregistered {
                        storage_fee_refund: account.storage_escrow.into(),
                    }
                }
            }
        }
    }

    fn registered_accounts_count(&self) -> U128 {
        self.accounts.count.into()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub enum RegisterAccountResult {
    AlreadyRegistered,
    Registered { storage_fee: YoctoNEAR },
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub enum UnregisterAccountResult {
    NotRegistered,
    /// account must first unstake and withdraw all funds before being able to unregister the account
    AccountHasFunds,
    Unregistered {
        storage_fee_refund: YoctoNEAR,
    },
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::stake::YOCTO;
    use crate::test_utils::near;
    use near_sdk::{testing_env, MockedBlockchain, VMContext};

    #[test]
    fn register_account_result_json() {
        let result = RegisterAccountResult::AlreadyRegistered;
        let json = serde_json::to_string_pretty(&result).unwrap();
        println!("RegisterAccountResult::AlreadyRegistered JSON: {}", json);

        let result = RegisterAccountResult::Registered {
            storage_fee: YoctoNEAR::from(YOCTO),
        };
        let json = serde_json::to_string_pretty(&result).unwrap();
        println!("RegisterAccountResult::Registered JSON: {}", json);
    }

    fn operator_id() -> AccountId {
        near::to_account_id("operator.stake.oysterpack.near")
    }

    #[test]
    fn register_new_account_success() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context);
        let mut contract = StakeTokenService::new(operator_id(), None);
        assert!(
            !contract.account_registered(account_id.clone()),
            "account should not be registered"
        );
        assert_eq!(
            contract.registered_accounts_count().0,
            0,
            "There should be no accounts registered"
        );

        match contract.register_account() {
            RegisterAccountResult::Registered { storage_fee } => {
                println!(
                    "account storage fee: {:?} NEAR",
                    storage_fee.0 as f64 / YOCTO as f64
                );
            }
            RegisterAccountResult::AlreadyRegistered => {
                panic!("account should not be already registered");
            }
        }

        assert!(
            contract.account_registered(account_id.clone()),
            "account should be registered"
        );
        assert_eq!(
            contract.registered_accounts_count().0,
            1,
            "There should be 1 account registered"
        );
    }

    #[test]
    fn register_preexisting_account() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context);
        let mut contract = StakeTokenService::new(operator_id(), None);

        match contract.register_account() {
            RegisterAccountResult::Registered { storage_fee } => {
                // when trying to register the same account again
                match contract.register_account() {
                    RegisterAccountResult::AlreadyRegistered => (), // expected
                    _ => panic!("expected AlreadyRegistered result"),
                }
            }
            RegisterAccountResult::AlreadyRegistered => {
                panic!("account should not be already registered");
            }
        }
    }

    #[test]
    #[should_panic]
    fn register_new_account_with_no_deposit() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        context.attached_deposit = 0;
        testing_env!(context);
        let mut contract = StakeTokenService::new(operator_id(), None);
        contract.register_account();
    }

    #[test]
    #[should_panic]
    fn register_new_account_with_not_enough_deposit() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        context.attached_deposit = 1;
        testing_env!(context);
        let mut contract = StakeTokenService::new(operator_id(), None);
        contract.register_account();
    }

    #[test]
    fn unregister_account_with_zero_funds() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context);
        let mut contract = StakeTokenService::new(operator_id(), None);
        match contract.register_account() {
            RegisterAccountResult::Registered { storage_fee } => {
                match contract.unregister_account() {
                    UnregisterAccountResult::Unregistered { storage_fee_refund } => {
                        assert_eq!(storage_fee.0, storage_fee_refund.0)
                    }
                    result => panic!("unexpected result: {:?}", result),
                }
            }
            _ => panic!("registration failed"),
        }
    }

    #[test]
    fn unregister_non_existent_account() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context);
        let mut contract = StakeTokenService::new(operator_id(), None);
        match contract.unregister_account() {
            UnregisterAccountResult::NotRegistered => (), // expected
            result => panic!("unexpected result: {:?}", result),
        }
    }

    #[test]
    fn unregister_account_with_near_funds() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context);
        let mut contract = StakeTokenService::new(operator_id(), None);
        match contract.register_account() {
            RegisterAccountResult::Registered { storage_fee } => {
                let account_hash = Hash::from(account_id.as_bytes());
                let mut account = contract.accounts.accounts.get(&account_hash).unwrap();
                account.available_near_balance = 10;
                contract.accounts.accounts.insert(&account_hash, &account);
                match contract.unregister_account() {
                    UnregisterAccountResult::AccountHasFunds => (), // expected
                    result => panic!("unexpected result: {:?}", result),
                }
            }
            _ => panic!("registration failed"),
        }
    }

    #[test]
    fn unregister_account_with_stake_funds() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context);
        let mut contract = StakeTokenService::new(operator_id(), None);
        match contract.register_account() {
            RegisterAccountResult::Registered { storage_fee } => {
                let account_hash = Hash::from(account_id.as_bytes());
                let mut account = contract.accounts.accounts.get(&account_hash).unwrap();
                account.stake_balances.insert(&account_id, &10);
                contract.accounts.accounts.insert(&account_hash, &account);
                match contract.unregister_account() {
                    UnregisterAccountResult::AccountHasFunds => (), // expected
                    result => panic!("unexpected result: {:?}", result),
                }
            }
            _ => panic!("registration failed"),
        }
    }
}
