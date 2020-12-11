use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};
use std::ops::Add;

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
    Hash,
    Default,
)]
#[serde(crate = "near_sdk::serde")]
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
