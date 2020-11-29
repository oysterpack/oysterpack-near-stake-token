// TODO: remove
#![allow(unused_imports, dead_code, unused_variables)]

pub mod account;
pub mod common;
pub mod config;
pub mod events;
pub mod stake;
pub mod state;

use crate::account::Accounts;
use crate::config::Config;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::wee_alloc;
use near_sdk::{env, near_bindgen};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// Structs in Rust are similar to other languages, and may include impl keyword as shown below
// Note: the names of the structs are not important when calling the smart contract, but the function names are
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakeTokenService {
    config: Config,
    accounts: Accounts,
}

impl Default for StakeTokenService {
    fn default() -> Self {
        unimplemented!()
    }
}

#[near_bindgen]
impl StakeTokenService {
    #[init]
    pub fn new(config: Option<Config>) -> Self {
        assert!(!env::state_exists(), "contract is already initialized");
        Self {
            config: config.unwrap_or_else(Config::default),
            accounts: Accounts::default(),
        }
    }
}
