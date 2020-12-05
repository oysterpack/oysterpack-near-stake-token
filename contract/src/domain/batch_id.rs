use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

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
    Default,
    Hash,
)]
#[serde(crate = "near_sdk::serde")]
pub struct BatchId(pub u64);

impl From<u64> for BatchId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl BatchId {
    pub fn value(&self) -> u64 {
        self.0
    }

    /// returns the next batch ID
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl From<BatchId> for u64 {
    fn from(value: BatchId) -> Self {
        value.0
    }
}
