use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};
use std::cmp::Ordering;
use std::ops::{AddAssign, Sub, SubAssign};

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

impl PartialOrd<StorageUsage> for u64 {
    fn partial_cmp(&self, other: &StorageUsage) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl PartialEq<StorageUsage> for u64 {
    fn eq(&self, other: &StorageUsage) -> bool {
        *self == other.0
    }
}

impl AsRef<u64> for StorageUsage {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}

impl Sub<StorageUsage> for u64 {
    type Output = u64;

    fn sub(self, rhs: StorageUsage) -> Self::Output {
        self - rhs.0
    }
}

impl AddAssign<StorageUsage> for StorageUsage {
    fn add_assign(&mut self, rhs: StorageUsage) {
        self.0 = self.0.checked_add(rhs.0).unwrap()
    }
}

impl SubAssign<StorageUsage> for StorageUsage {
    fn sub_assign(&mut self, rhs: StorageUsage) {
        self.0 = self.0.checked_sub(rhs.0).unwrap()
    }
}
