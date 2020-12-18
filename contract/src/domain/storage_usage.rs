use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

use std::ops::{AddAssign, Deref, DerefMut, Sub, SubAssign};

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

impl Deref for StorageUsage {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StorageUsage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
