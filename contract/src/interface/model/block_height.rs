use crate::domain;
use near_sdk::{
    json_types::U64,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct BlockHeight(pub U64);

impl From<domain::BlockHeight> for BlockHeight {
    fn from(value: domain::BlockHeight) -> Self {
        Self(value.0.into())
    }
}
