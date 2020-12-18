
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
};

/// Lock lifecycle transitions: [Unstaking] -> [PendingWithdrawal] -> [WithdrawalComplete]
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum RedeemLock {
    Unstaking,
    /// while locked on pending withdrawal of unstaked funds, the receipt for the specified
    /// batch ID cannot be claimed
    PendingWithdrawal,
}
