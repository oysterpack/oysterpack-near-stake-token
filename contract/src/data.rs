//! This module and its defines the data model used for persistent contract object storage on the
//! NEAR blockchain.
//!
//! All objects are persisted using [Borsh] serialization.
//!
//! [Borsh]: https://crates.io/crates/borsh

use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env, Balance, BlockHeight, EpochHeight,
};
use std::cmp::Ordering;

pub mod accounts;
pub mod activities;
pub mod config;
pub mod staking_pools;
pub mod trie_keys;

pub use trie_keys::*;

pub type BlockTimestamp = u64;

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, Copy)]
pub struct TimestampedBalance {
    balance: Balance,
    block_height: BlockHeight,
    block_timestamp: BlockTimestamp,
    epoch_height: EpochHeight,
}

impl Default for TimestampedBalance {
    /// ## Panics
    /// if NEAR runtime env is not available
    fn default() -> Self {
        Self::new(0)
    }
}

impl PartialEq for TimestampedBalance {
    fn eq(&self, other: &Self) -> bool {
        self.balance == other.balance
    }
}

impl PartialEq<u128> for TimestampedBalance {
    fn eq(&self, other: &u128) -> bool {
        self.balance == *other
    }
}

impl PartialOrd for TimestampedBalance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.balance.cmp(&other.balance))
    }
}

impl PartialOrd<u128> for TimestampedBalance {
    fn partial_cmp(&self, other: &u128) -> Option<Ordering> {
        Some(self.balance.cmp(other))
    }
}

impl TimestampedBalance {
    /// [block_height], [block_timestamp], and [epoch_height] are initialized from the NEAR runtime
    /// environment
    ///
    /// ## Panics
    /// if NEAR runtime context is not available
    pub fn new(balance: Balance) -> Self {
        Self {
            balance,
            block_height: env::block_index(),
            block_timestamp: env::block_timestamp(),
            epoch_height: env::epoch_height(),
        }
    }

    pub fn balance(&self) -> Balance {
        self.balance
    }

    pub fn block_height(&self) -> BlockHeight {
        self.block_height
    }

    pub fn block_timestamp(&self) -> BlockTimestamp {
        self.block_timestamp
    }

    pub fn epoch_height(&self) -> EpochHeight {
        self.epoch_height
    }

    /// ## Panics
    /// if overflow occurs
    pub fn credit(&mut self, amount: Balance) {
        self.balance = self.balance.checked_add(amount).expect(
            format!(
                "credit caused balance to overflow: {balance} + {amount}",
                amount = amount,
                balance = self.balance
            )
            .as_str(),
        );
        self.update_timestamp();
    }

    /// ## Panics
    /// if debit amount > balance
    pub fn debit(&mut self, amount: Balance) {
        assert!(
            self.balance > amount,
            "debit amount cannot be greater than the current balance: {} - {}",
            self.balance,
            amount,
        );
        self.balance -= amount;
        self.update_timestamp();
    }

    fn update_timestamp(&mut self) {
        self.epoch_height = env::epoch_height();
        self.block_timestamp = env::block_timestamp();
        self.block_height = env::block_index();
    }
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Hash([u8; 32]);

impl Hash {
    const LENGTH: usize = 32;
}

impl From<&[u8]> for Hash {
    fn from(value: &[u8]) -> Self {
        let mut buf = [0u8; Hash::LENGTH];
        let hash = env::sha256(value);
        buf.copy_from_slice(&hash.as_slice()[..Hash::LENGTH]);
        Self(buf)
    }
}

impl From<&str> for Hash {
    fn from(value: &str) -> Self {
        let mut buf = [0u8; Hash::LENGTH];
        let hash = env::sha256(value.as_bytes());
        buf.copy_from_slice(&hash.as_slice()[..Hash::LENGTH]);
        Self(buf)
    }
}

#[cfg(test)]
mod test {
    use crate::test_utils::near::new_context;
    use near_sdk::{
        borsh::{BorshDeserialize, BorshSerialize},
        testing_env, MockedBlockchain, VMContext,
    };

    use super::*;

    #[test]
    #[should_panic]
    fn timestamped_balance_new_outside_near_runtime() {
        let _balance = TimestampedBalance::new(10);
    }

    #[test]
    fn timestamped_balance_new() {
        let mut context = new_context("bob.near".to_string());
        context.block_index = 1;
        context.block_timestamp = 2;
        context.epoch_height = 3;

        testing_env!(context);
        let balance = TimestampedBalance::new(10);
        assert_eq!(balance.balance(), 10);
        assert_eq!(balance.block_height(), 1);
        assert_eq!(balance.block_timestamp(), 2);
        assert_eq!(balance.epoch_height(), 3);
    }

    #[test]
    fn timestamped_balance_credit() {
        let mut context = new_context("bob.near".to_string());
        context.block_index = 1;
        context.block_timestamp = 2;
        context.epoch_height = 3;
        testing_env!(context.clone());

        let mut balance = TimestampedBalance::new(10);
        assert_eq!(balance.balance(), 10);
        assert_eq!(balance.block_height(), 1);
        assert_eq!(balance.block_timestamp(), 2);
        assert_eq!(balance.epoch_height(), 3);

        context.block_index = 10;
        context.block_timestamp = 20;
        context.epoch_height = 30;
        testing_env!(context.clone());

        balance.credit(10);
        assert_eq!(balance.balance(), 20);
        assert_eq!(balance.block_height(), 10);
        assert_eq!(balance.block_timestamp(), 20);
        assert_eq!(balance.epoch_height(), 30);
    }

    #[test]
    fn timestamped_balance_debit() {
        let mut context = new_context("bob.near".to_string());
        context.block_index = 1;
        context.block_timestamp = 2;
        context.epoch_height = 3;
        testing_env!(context.clone());

        let mut balance = TimestampedBalance::new(10);
        assert_eq!(balance.balance(), 10);
        assert_eq!(balance.block_height(), 1);
        assert_eq!(balance.block_timestamp(), 2);
        assert_eq!(balance.epoch_height(), 3);

        context.block_index = 10;
        context.block_timestamp = 20;
        context.epoch_height = 30;
        testing_env!(context.clone());

        balance.debit(5);
        assert_eq!(balance.balance(), 5);
        assert_eq!(balance.block_height(), 10);
        assert_eq!(balance.block_timestamp(), 20);
        assert_eq!(balance.epoch_height(), 30);
    }

    #[test]
    #[should_panic]
    fn timestamped_balance_debit_more_than_balance() {
        let context = new_context("bob.near".to_string());
        testing_env!(context);

        let mut balance = TimestampedBalance::new(10);
        balance.debit(balance.balance + 1);
    }

    #[test]
    #[should_panic]
    fn timestamped_balance_credit_overflow() {
        let context = new_context("bob.near".to_string());
        testing_env!(context);

        let mut balance = TimestampedBalance::new(u128::MAX);
        balance.debit(balance.balance + 1);
    }

    #[test]
    fn timestamped_balance_default() {
        let mut context = new_context("bob.near".to_string());
        context.block_index = 1;
        context.block_timestamp = 2;
        context.epoch_height = 3;
        testing_env!(context);

        let balance = TimestampedBalance::default();
        assert_eq!(balance.balance(), 0);
        assert_eq!(balance.block_height(), 1);
        assert_eq!(balance.block_timestamp(), 2);
        assert_eq!(balance.epoch_height(), 3);
    }

    #[test]
    #[should_panic]
    fn timestamped_balance_default_outside_near_runtime() {
        let _balance = TimestampedBalance::default();
    }

    #[test]
    pub fn timestamped_balance_partial_ord() {
        let context = new_context("bob.near".to_string());
        testing_env!(context);

        let balance_10 = TimestampedBalance::new(10);
        let balance_20 = TimestampedBalance::new(20);

        assert!(balance_10 < balance_20);
        assert!(balance_10 < 20u128);
    }

    #[test]
    pub fn timestamped_balance_partial_eq() {
        let mut context = new_context("bob.near".to_string());
        testing_env!(context.clone());

        let balance_1 = TimestampedBalance::new(10);

        context.block_index = 10;
        context.block_timestamp = 20;
        context.epoch_height = 30;
        testing_env!(context.clone());
        let balance_2 = TimestampedBalance::new(10);

        assert!(balance_1 == balance_2);
        assert!(balance_1 == 10u128);
    }

    #[test]
    pub fn timestamped_balance_debug() {
        let mut context = new_context("bob.near".to_string());
        context.block_index = 1;
        context.block_timestamp = 2;
        context.epoch_height = 3;
        testing_env!(context.clone());

        let balance = TimestampedBalance::new(10);
        println!("{:?}", balance);
    }

    #[test]
    pub fn timestamped_balance_borsh() {
        let mut context = new_context("bob.near".to_string());
        context.block_index = 1;
        context.block_timestamp = 2;
        context.epoch_height = 3;
        testing_env!(context.clone());

        let balance = TimestampedBalance::new(10);
        let bytes: Vec<u8> = balance.try_to_vec().unwrap();
        let balance2: TimestampedBalance = TimestampedBalance::try_from_slice(&bytes).unwrap();
        assert_eq!(balance, balance2);
    }

    #[test]
    fn hash_from_string() {
        let account_id = "alfio-zappala.near".to_string();
        let context = new_context(account_id.clone());
        testing_env!(context);
        let data = "Alfio Zappala";
        let hash = Hash::from(data);
        let hash2 = Hash::from(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn hash_from_bytes() {
        let account_id = "alfio-zappala.near".to_string();
        let context = new_context(account_id.clone());
        testing_env!(context);
        let data = "Alfio Zappala II";
        let hash = Hash::from(data.as_bytes());
        let hash2 = Hash::from(data.as_bytes());
        assert_eq!(hash, hash2);
    }
}
