//! This module is used to centralize NEAR SDK Collection IDs to ensure duplicates are not defined
//!
//! Each NEAR SDK persistent collection must be defined with a unique ID, which is used to store the
//! collection in the TRIE. Each of the IDs defined below should only be referenced once within the
//! project.

pub const ACCOUNTS_KEY_PREFIX: [u8; 1] = [0];
pub const STAKE_BATCH_RECEIPTS_KEY_PREFIX: [u8; 1] = [1];
pub const REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX: [u8; 1] = [2];
pub const UNCLAIMED_STAKE_BATCH_FUNDS_KEY_PREFIX: [u8; 1] = [3];
pub const UNCLAIMED_REDEEME_STAKE_BATCH_FUNDS_KEY_PREFIX: [u8; 1] = [4];
