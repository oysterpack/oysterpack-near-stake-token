use crate::domain::BatchId;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum RedeemLock {
    Unstaking,
    PendingWithdrawal(BatchId),
}
