use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
};
use std::ops::Add;

#[derive(
    BorshSerialize,
    BorshDeserialize,
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Default,
)]
pub struct EpochHeight(pub u64);

impl From<u64> for EpochHeight {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl EpochHeight {
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl From<EpochHeight> for u64 {
    fn from(value: EpochHeight) -> Self {
        value.0
    }
}

impl Add<u64> for EpochHeight {
    type Output = EpochHeight;

    fn add(self, rhs: u64) -> Self::Output {
        EpochHeight(self.0 + rhs)
    }
}

impl Add<EpochHeight> for EpochHeight {
    type Output = EpochHeight;

    fn add(self, rhs: EpochHeight) -> Self::Output {
        EpochHeight(self.0 + rhs.0)
    }
}
