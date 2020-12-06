use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};
use std::ops::{Deref, DerefMut};

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
    Hash,
)]
#[serde(crate = "near_sdk::serde")]
pub struct BatchId(pub u64);

impl From<u64> for BatchId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl BatchId {
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl From<BatchId> for u64 {
    fn from(value: BatchId) -> Self {
        value.0
    }
}

impl Deref for BatchId {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BatchId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod test {
    use crate::domain::BatchId;

    #[test]
    fn batch_id_deref() {
        let n = 10u64;
        let batch_id = BatchId::default();
        let x = n + *batch_id;

        fn foo(bar: u64) -> u64 {
            bar
        }

        foo(*batch_id);
    }

    #[test]
    fn batch_inc() {
        let mut batch_id = BatchId::default();
        *batch_id += 1;
        assert_eq!(*batch_id, 1);
    }
}
