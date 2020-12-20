use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};
use std::ops::{Add, Mul};

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
pub struct Gas(pub u64);

impl From<u64> for Gas {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl Gas {
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl Add for Gas {
    type Output = Gas;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Mul<u64> for Gas {
    type Output = Gas;

    fn mul(self, rhs: u64) -> Self::Output {
        Self(self.0 * rhs)
    }
}
