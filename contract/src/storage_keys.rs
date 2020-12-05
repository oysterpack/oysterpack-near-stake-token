//! This module is used to centralize NEAR SDK Collection IDs to ensure duplicates are not defined
//!
//! Each NEAR SDK persistent collection must be defined with a unique ID, which is used to store the
//! collection in the TRIE. Each of the IDs defined below should only be referenced once within the
//! project.

pub const ACCOUNTS_KEY_PREFIX: [u8; 1] = [0];
pub const ACCOUNT_STAKE_BALANCES_KEY_PREFIX: [u8; 1] = [1];
pub const DEPOSIT_AND_STAKE_ACTIVITY_KEY_PREFIX: [u8; 1] = [2];
