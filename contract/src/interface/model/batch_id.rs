use crate::domain;
use near_sdk::{
    json_types::U128,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct BatchId(pub U128);

impl From<domain::BatchId> for BatchId {
    fn from(value: domain::BatchId) -> Self {
        Self(value.0.into())
    }
}

impl From<BatchId> for u128 {
    fn from(vale: BatchId) -> Self {
        vale.0 .0
    }
}
