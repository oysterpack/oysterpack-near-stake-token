use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::export::Formatter;
use std::fmt::{self, Display};

#[derive(
    BorshSerialize, BorshDeserialize, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default,
)]
pub struct YoctoStake(pub u128);

impl From<u128> for YoctoStake {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

impl YoctoStake {
    pub fn value(&self) -> u128 {
        self.0
    }
}

impl From<YoctoStake> for u128 {
    fn from(value: YoctoStake) -> Self {
        value.0
    }
}

impl Display for YoctoStake {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
