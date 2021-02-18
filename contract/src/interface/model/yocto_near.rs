use crate::domain;
use near_sdk::{
    json_types::U128,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct YoctoNear(pub U128);

impl From<domain::YoctoNear> for YoctoNear {
    fn from(value: domain::YoctoNear) -> Self {
        Self(value.0.into())
    }
}

impl From<u128> for YoctoNear {
    fn from(value: u128) -> Self {
        Self(value.into())
    }
}

impl YoctoNear {
    pub fn value(&self) -> u128 {
        self.0 .0
    }
}

impl Default for YoctoNear {
    fn default() -> Self {
        Self(U128(0))
    }
}
