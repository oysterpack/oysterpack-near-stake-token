#![allow(dead_code, unused_variables, unused_imports)]

use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::LookupMap,
    env, ext_contract,
    json_types::{ValidAccountId, U128, U64},
    log, near_bindgen,
    serde::{Deserialize, Serialize},
    wee_alloc, AccountId, Balance, EpochHeight, PanicOnDefault, Promise, PromiseOrValue,
    PromiseResult,
};
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

/// staking pool interface that STAKE token contract depends on
#[near_bindgen]
impl StakingPool {
    #[init]
    pub fn new() -> Self {
        Self {
            accounts: LookupMap::new(vec![1]),
        }
    }

    pub fn get_account(&self, account_id: AccountId) -> StakingPoolAccount {
        log!("StakingPool::get_account()");
        self.accounts
            .get(&account_id)
            .unwrap_or_else(|| StakingPoolAccount::new(&account_id))
    }

    #[payable]
    pub fn deposit(&mut self) {
        log!("StakingPool::deposit()");
        let mut account = self.get_account(env::predecessor_account_id());
        account.unstaked_balance = (account.unstaked_balance.0 + env::attached_deposit()).into();
        self.save_account(&account);
    }

    pub fn stake(&mut self, amount: U128) {
        log!("StakingPool::stake()");
        let mut account = self.get_account(env::predecessor_account_id());
        account.unstaked_balance = (account.unstaked_balance.0 - amount.0).into();
        account.staked_balance = (account.staked_balance.0 + amount.0).into();
        self.save_account(&account);
    }

    #[payable]
    pub fn deposit_and_stake(&mut self) {
        log!("StakingPool::deposit_and_stake()");
        self.deposit();
        self.stake(env::attached_deposit().into());
    }

    pub fn withdraw_all(&mut self) {
        log!("StakingPool::withdraw_all()");
        let mut account = self.get_account(env::predecessor_account_id());
        assert!(account.can_withdraw, "account cannot withdraw yet");
        assert!(account.unstaked_balance.0 > 0, "unstaked balance is zero");
        let unstaked_balance = account.unstaked_balance.0;
        account.unstaked_balance = 0.into();
        self.save_account(&account);
        Promise::new(env::predecessor_account_id()).transfer(unstaked_balance);
    }

    pub fn unstake(&mut self, amount: U128) {
        log!("StakingPool::unstake()");
        let mut account = self.get_account(env::predecessor_account_id());
        assert!(account.staked_balance.0 >= amount.0);
        account.staked_balance = (account.staked_balance.0 - amount.0).into();
        account.unstaked_balance = (account.unstaked_balance.0 + amount.0).into();
        self.save_account(&account);
    }

    pub fn unstake_all(&mut self) {
        log!("StakingPool::unstake_all()");
        let mut account = self.get_account(env::predecessor_account_id());
        assert!(account.staked_balance.0 > 0, "staked balance is zero");
        account.unstaked_balance = (account.unstaked_balance.0 + account.staked_balance.0).into();
        account.staked_balance = 0.into();
        self.save_account(&account);
    }
}

/// exposed to support simulation testing
#[near_bindgen]
impl StakingPool {
    pub fn update_account(&mut self, account: StakingPoolAccount) {
        self.save_account(&account);
    }
}

impl StakingPool {
    fn save_account(&mut self, account: &StakingPoolAccount) {
        self.accounts.insert(&account.account_id, account);
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
            can_withdraw: false,
        }
    }
}
