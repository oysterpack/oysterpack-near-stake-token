#![allow(dead_code, unused_variables, unused_imports)]

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::{ValidAccountId, U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, PanicOnDefault, PromiseOrValue};
use near_sdk::{wee_alloc, AccountId, Promise, PromiseResult};
use near_sdk::{Balance, EpochHeight};
use std::convert::TryFrom;

// uncomment to build wasm file
// comment out to run sim-tests
// #[global_allocator]
// static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct StakingPool {
    accounts: LookupMap<String, StakingPoolAccount>,
}

#[near_bindgen]
impl StakingPool {
    #[init]
    pub fn new() -> Self {
        Self {
            accounts: LookupMap::new(vec![1]),
        }
    }

    pub fn get_account(&self, account_id: AccountId) -> StakingPoolAccount {
        self.accounts
            .get(&account_id)
            .unwrap_or_else(|| StakingPoolAccount::new(&account_id))
    }

    #[payable]
    pub fn deposit(&mut self) {
        let mut account = self.get_account(env::predecessor_account_id());
        account.unstaked_balance = (account.unstaked_balance.0 + env::attached_deposit()).into();
        self.save_account(&account);
    }

    pub fn stake(&mut self, amount: U128) {
        let mut account = self.get_account(env::predecessor_account_id());
        account.unstaked_balance = (account.unstaked_balance.0 - amount.0).into();
        account.staked_balance = (account.staked_balance.0 + amount.0).into();
        self.save_account(&account);
    }

    #[payable]
    pub fn deposit_and_stake(&mut self) {
        self.deposit();
        self.stake(env::attached_deposit().into());
    }

    pub fn withdraw_all(&mut self) {
        let mut account = self.get_account(env::predecessor_account_id());
        assert!(account.can_withdraw, "account cannot withdraw yet");
        assert!(account.unstaked_balance.0 > 0, "unstaked balance is zero");
        let unstaked_balance = account.unstaked_balance.0;
        account.unstaked_balance = 0.into();
        self.save_account(&account);
        Promise::new(env::predecessor_account_id()).transfer(unstaked_balance);
    }

    pub fn unstake(&mut self, amount: U128) {
        let mut account = self.get_account(env::predecessor_account_id());
        assert!(account.staked_balance.0 >= amount.0);
        account.staked_balance = (account.staked_balance.0 - amount.0).into();
        account.unstaked_balance = (account.unstaked_balance.0 + amount.0).into();
        self.save_account(&account);
    }

    pub fn unstake_all(&mut self) {
        let mut account = self.get_account(env::predecessor_account_id());
        assert!(account.staked_balance.0 > 0, "staked balance is zero");
        account.unstaked_balance = (account.unstaked_balance.0 + account.staked_balance.0).into();
        account.staked_balance = 0.into();
        self.save_account(&account);
    }
}

impl StakingPool {
    fn save_account(&mut self, account: &StakingPoolAccount) {
        self.accounts
            .insert(&env::predecessor_account_id(), account);
    }
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolAccount {
    pub account_id: AccountId,
    /// The unstaked balance that can be withdrawn or staked.
    pub unstaked_balance: U128,
    /// The amount balance staked at the current "stake" share price.
    pub staked_balance: U128,
    /// Whether the unstaked balance is available for withdrawal now.
    pub can_withdraw: bool,
}

impl StakingPoolAccount {
    pub fn new(account_id: &str) -> Self {
        Self {
            account_id: account_id.to_string(),
            unstaked_balance: U128(0),
            staked_balance: U128(0),
            can_withdraw: true,
        }
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn quick_test() {}
}
