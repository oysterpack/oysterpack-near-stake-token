use near_sdk::json_types::U64;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::U128,
    serde::{self, Deserialize, Serialize},
};

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
    storage_cost_per_byte: U128,
    gas_config: GasConfig,
}

impl Config {
    pub fn new(storage_cost_per_byte: u128, gas_config: GasConfig) -> Self {
        Self {
            storage_cost_per_byte: storage_cost_per_byte.into(),
            gas_config,
        }
    }

    pub fn storage_cost_per_byte(&self) -> u128 {
        self.storage_cost_per_byte.into()
    }

    pub fn gas_config(&self) -> &GasConfig {
        &self.gas_config
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // 1E20 yoctoNEAR (0.00001 NEAR) per byte or 10kb per NEAR token
            // https://docs.near.org/docs/concepts/storage
            storage_cost_per_byte: 100_000_000_000_000_000_000.into(),
            gas_config: GasConfig::default(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GasConfig {
    deposit_and_stake: U64,
    unstake: U64,
    withdraw: U64,
    get_account_balance: U64,

    // callbacks
    on_deposit_and_stake: U64,
}

impl Default for GasConfig {
    fn default() -> Self {
        const BASE_GAS: u64 = 25_000_000_000_000;
        Self {
            deposit_and_stake: (BASE_GAS * 3).into(),
            unstake: (BASE_GAS * 3).into(),
            withdraw: (BASE_GAS * 3).into(),
            get_account_balance: BASE_GAS.into(),
            on_deposit_and_stake: BASE_GAS.into(),
        }
    }
}

impl GasConfig {
    pub fn deposit_and_stake(&self) -> u64 {
        self.deposit_and_stake.0
    }

    pub fn unstake(&self) -> u64 {
        self.unstake.0
    }

    pub fn withdraw(&self) -> u64 {
        self.withdraw.0
    }

    pub fn get_account_balance(&self) -> u64 {
        self.get_account_balance.0
    }

    pub fn on_deposit_and_stake(&self) -> u64 {
        self.on_deposit_and_stake.0
    }
}
