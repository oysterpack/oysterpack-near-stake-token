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
)]
#[serde(crate = "near_sdk::serde")]
pub struct StorageUsage(pub u64);

impl From<u64> for StorageUsage {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl StorageUsage {
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl From<StorageUsage> for u64 {
    fn from(value: StorageUsage) -> Self {
        value.0
    }
}
