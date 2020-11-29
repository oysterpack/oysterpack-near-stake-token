use crate::common::{Hash, YoctoNEAR};
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
    stake_balances: UnorderedMap<AccountId, Balance>,
    near_balance: Balance,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            storage_escrow: 0,
            near_balance: 0,
            stake_balances: UnorderedMap::new(b"c".to_vec()),
        }
    }
}

trait AccountRepository {
    fn account_exists(&self, account_id: AccountId) -> bool;

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
    ///
    ///
    fn unregister_account(&mut self) -> UnregisterAccountResult;

    fn registered_accounts_count(&self) -> U128;
}

#[near_bindgen]
impl AccountRepository for StakeTokenService {
    fn account_exists(&self, account_id: AccountId) -> bool {
        self.accounts
            .accounts
            .contains_key(&Hash::from(account_id.as_str()))
    }

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
        let account = Account::default();
        self.accounts.accounts.insert(&account_hash, &account);
        // this has the potential to overflow in the far distant future ...
        self.accounts.count += 1;

        let storage_fee = apply_storage_fees(self, initial_storage_usage);
        RegisterAccountResult::Registered {
            storage_fee: storage_fee.into(),
        }
    }

    fn unregister_account(&mut self) -> UnregisterAccountResult {
        unimplemented!()
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
    Unregistered { storage_fee_refund: YoctoNEAR },
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::stake::YOCTO;

    const CONTRACT_ACCOUNT: &str = "stake.oysterpack.near";

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

    #[test]
    fn register_new_account_success() {
        let account_id = "alfio-zappala.near".to_string();
    }

    #[test]
    fn register_preexisting_account() {
        unimplemented!()
    }

    #[test]
    fn register_new_account_with_no_deposit() {
        unimplemented!()
    }

    #[test]
    fn register_new_account_with_not_enough_deposit() {
        unimplemented!()
    }
}
