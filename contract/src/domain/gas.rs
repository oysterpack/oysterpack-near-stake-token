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

impl From<Gas> for u64 {
    fn from(value: Gas) -> Self {
        value.0
    }
}
