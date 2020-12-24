use crate::interface;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use std::ops::{Deref, DerefMut};

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
    Hash,
)]
pub struct BatchId(pub u128);

impl From<u128> for BatchId {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

impl BatchId {
    pub fn value(&self) -> u128 {
        self.0
    }
}

impl From<BatchId> for u128 {
    fn from(value: BatchId) -> Self {
        value.0
    }
}

impl From<interface::BatchId> for BatchId {
    fn from(value: interface::BatchId) -> Self {
        BatchId(value.0 .0)
    }
}

impl Deref for BatchId {
    type Target = u128;

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
        let n = 10u128;
        let batch_id = BatchId::default();
        let _x = n + *batch_id;

        fn foo(bar: u128) -> u128 {
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
