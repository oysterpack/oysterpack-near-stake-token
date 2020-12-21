use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

/// Lock lifecycle transitions: [Unstaking] -> [PendingWithdrawal] -> [WithdrawalComplete]
#[derive(
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
)]
#[serde(crate = "near_sdk::serde")]
pub enum RedeemLock {
    Unstaking,
    /// while locked on pending withdrawal of unstaked funds, the receipt for the specified
    /// batch ID cannot be claimed
    PendingWithdrawal,
}
