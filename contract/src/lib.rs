// TODO: remove
#![allow(unused_imports, dead_code, unused_variables)]

pub mod config;
pub mod contract;
mod core;
pub mod domain;
pub mod interface;
pub mod near;

#[cfg(test)]
pub mod test_utils;

use crate::config::Config;
use crate::core::Hash;
use crate::domain::{Account, YoctoNear};
use near_sdk::collections::LookupMap;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, near_bindgen, wee_alloc, AccountId, BlockHeight, StorageUsage,
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakeTokenContract {
    /// Operator is allowed to perform operator actions on the contract
    /// TODO: support multiple operator and role management
    operator_id: AccountId,

    config: Config,
    /// when the config was last changed
    /// the block info can be looked up via its block index: https://docs.near.org/docs/api/rpc#block
    config_change_block_height: BlockHeight,

    accounts: LookupMap<Hash, Account>,
    account_count: u128,
}
