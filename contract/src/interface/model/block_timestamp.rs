use crate::domain;
use near_sdk::{
    json_types::U64,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct BlockTimestamp(pub U64);

impl From<domain::BlockTimestamp> for BlockTimestamp {
    fn from(value: domain::BlockTimestamp) -> Self {
        Self(value.0.into())
    }
}
