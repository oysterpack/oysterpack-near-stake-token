use crate::domain;
use near_sdk::{
    json_types::U128,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct YoctoStake(pub U128);

impl From<domain::YoctoStake> for YoctoStake {
    fn from(value: domain::YoctoStake) -> Self {
        Self(value.0.into())
    }
}

impl From<u128> for YoctoStake {
    fn from(value: u128) -> Self {
        Self(value.into())
    }
}

impl YoctoStake {
    pub fn value(&self) -> u128 {
        self.0 .0
    }
}
