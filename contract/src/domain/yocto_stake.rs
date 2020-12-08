use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::export::Formatter;
use primitive_types::U256;
use std::fmt::{self, Display};
use std::ops::{Deref, DerefMut};

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

impl Deref for YoctoStake {
    type Target = u128;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for YoctoStake {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Display for YoctoStake {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<YoctoStake> for U256 {
    fn from(value: YoctoStake) -> Self {
        U256::from(value.value())
    }
}
