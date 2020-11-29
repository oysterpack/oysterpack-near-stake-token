use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use primitive_types::U128;

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
    storage_cost_per_byte: U128,
}

impl Config {
    pub fn storage_cost_per_byte(&self) -> u128 {
        self.storage_cost_per_byte.into()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // 1E20 yoctoNEAR (0.00001 NEAR) per byte or 10kb per NEAR token
            // https://docs.near.org/docs/concepts/storage
            storage_cost_per_byte: 100_000_000_000_000_000_000.into(),
        }
    }
}
