use crate::domain;
use near_sdk::{
    json_types::U64,
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct StorageUsage(pub U64);

impl From<domain::StorageUsage> for StorageUsage {
    fn from(value: domain::StorageUsage) -> Self {
        value.0.into()
    }
}

impl From<u64> for StorageUsage {
    fn from(value: u64) -> Self {
        Self(value.into())
    }
}
