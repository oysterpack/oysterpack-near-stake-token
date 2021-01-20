use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
};

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
pub struct BlockTimestamp(pub u64);

impl From<u64> for BlockTimestamp {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl BlockTimestamp {
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl From<BlockTimestamp> for u64 {
    fn from(value: BlockTimestamp) -> Self {
        value.0
    }
}
