use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::export::Formatter;
use std::fmt::{self, Display};

#[derive(
    BorshSerialize, BorshDeserialize, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default,
)]
pub struct YoctoNear(pub u128);

impl From<u128> for YoctoNear {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

impl YoctoNear {
    pub fn value(&self) -> u128 {
        self.0
    }
}

impl From<YoctoNear> for u128 {
    fn from(value: YoctoNear) -> Self {
        value.0
    }
}

impl Display for YoctoNear {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
