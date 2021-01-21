use crate::core::U256;
use crate::interface;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::U128,
};
use std::{
    fmt::{self, Display, Formatter},
    ops::{Add, AddAssign, Deref, DerefMut, Sub, SubAssign},
};

#[derive(
    BorshSerialize, BorshDeserialize, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Default,
)]
pub struct YoctoNear(pub u128);

impl From<u128> for YoctoNear {
    fn from(value: u128) -> Self {
        Self(value)
    }
}

impl From<U128> for YoctoNear {
    fn from(value: U128) -> Self {
        Self(value.0)
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

impl From<YoctoNear> for U128 {
    fn from(value: YoctoNear) -> Self {
        value.0.into()
    }
}

impl Deref for YoctoNear {
    type Target = u128;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for YoctoNear {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Display for YoctoNear {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Sub for YoctoNear {
    type Output = YoctoNear;

    fn sub(self, rhs: Self) -> Self::Output {
        YoctoNear(
            self.0
                .checked_sub(rhs.0)
                .expect("attempt to subtract with overflow"),
        )
    }
}

impl SubAssign for YoctoNear {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self
            .0
            .checked_sub(rhs.0)
            .expect("attempt to subtract with overflow")
    }
}

impl Add for YoctoNear {
    type Output = YoctoNear;

    fn add(self, rhs: Self) -> Self::Output {
        YoctoNear(
            self.0
                .checked_add(rhs.0)
                .expect("attempt to add with overflow"),
        )
    }
}

impl AddAssign for YoctoNear {
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self
            .0
            .checked_add(rhs.0)
            .expect("attempt to add with overflow")
    }
}

impl From<interface::YoctoNear> for YoctoNear {
    fn from(value: interface::YoctoNear) -> Self {
        YoctoNear(value.value())
    }
}

impl From<YoctoNear> for U256 {
    fn from(value: YoctoNear) -> Self {
        U256::from(value.value())
    }
}
