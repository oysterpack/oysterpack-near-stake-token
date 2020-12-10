use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

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
pub struct BlockHeight(pub u64);

impl From<u64> for BlockHeight {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl BlockHeight {
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl From<BlockHeight> for u64 {
    fn from(value: BlockHeight) -> Self {
        value.0
    }
}
