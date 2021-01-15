#![allow(dead_code, unused_variables, unused_imports)]

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::{ValidAccountId, U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, PanicOnDefault, PromiseOrValue};
use near_sdk::{wee_alloc, AccountId, Promise, PromiseResult};
use std::convert::TryFrom;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

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
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolAccount {
    pub account_id: AccountId,
    /// The unstaked balance that can be withdrawn or staked.
    pub unstaked_balance: near_sdk::json_types::U128,
    /// The amount balance staked at the current "stake" share price.
    pub staked_balance: near_sdk::json_types::U128,
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
