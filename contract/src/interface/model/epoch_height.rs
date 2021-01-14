use crate::domain;
use near_sdk::{
    json_types::U64,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct EpochHeight(pub U64);

impl From<domain::EpochHeight> for EpochHeight {
    fn from(value: domain::EpochHeight) -> Self {
        Self(value.0.into())
    }
}
