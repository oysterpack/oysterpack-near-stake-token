use crate::domain;
use near_sdk::{
    json_types::U64,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct Gas(pub U64);

impl From<domain::Gas> for Gas {
    fn from(value: domain::Gas) -> Self {
        value.0.into()
    }
}

impl From<u64> for Gas {
    fn from(value: u64) -> Self {
        Self(value.into())
    }
}
