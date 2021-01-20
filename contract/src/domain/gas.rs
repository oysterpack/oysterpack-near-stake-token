use crate::interface;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
};
use std::ops::{Add, Mul};

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
    Default,
)]
pub struct Gas(pub u64);

/// 1 teraGas
pub const TGAS: Gas = Gas(1_000_000_000_000);

impl From<u64> for Gas {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<interface::Gas> for Gas {
    fn from(value: interface::Gas) -> Self {
        Self(value.0 .0)
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
