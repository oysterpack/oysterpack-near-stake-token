//! This module is used to centralize NEAR SDK Collection IDs to ensure duplicates are not defined
//!
//! Each NEAR SDK persistent collection must be defined with a unique ID, which is used to store the
//! collection in the TRIE. Each of the IDs defined below should only be referenced once within the
//! project.

pub(crate) const ACCOUNTS_STATE_ID: [u8; 1] = [0];
pub(crate) const STAKE_BALANCES_STATE_ID: [u8; 1] = [1];
pub(crate) const STAKING_POOLS_STATE_ID: [u8; 1] = [2];
