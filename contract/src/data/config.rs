use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use std::str::FromStr;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct Config {
    storage_cost_per_byte: u128,
    gas_config: GasConfig,
}

impl Config {
    pub fn new(storage_cost_per_byte: u128, gas_config: GasConfig) -> Self {
        Self {
            storage_cost_per_byte,
            gas_config,
        }
    }

    pub fn storage_cost_per_byte(&self) -> u128 {
        self.storage_cost_per_byte
    }

    pub fn gas_config(&self) -> &GasConfig {
        &self.gas_config
    }

    /// ## Panics
    /// if config is invalid
    pub fn apply_updates(&mut self, config: &updates::Config) {
        if let Some(storage_cost_per_byte) = config.storage_cost_per_byte.as_ref() {
            self.storage_cost_per_byte = u128::from_str(storage_cost_per_byte).unwrap();
        }

        if let Some(gas_config) = config.gas_config.as_ref() {
            self.gas_config.update(gas_config);
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // 1E20 yoctoNEAR (0.00001 NEAR) per byte or 10kb per NEAR token
            // https://docs.near.org/docs/concepts/storage
            storage_cost_per_byte: 100_000_000_000_000_000_000,
            gas_config: GasConfig::default(),
        }
    }
}

const BASE_GAS: u64 = 25_000_000_000_000;

#[derive(Debug, BorshSerialize, BorshDeserialize, Default)]
pub struct GasConfig {
    staking_pool: StakingPoolGasConfig,
    callbacks: CallBacksGasConfig,
}

impl GasConfig {
    pub fn staking_pool(&self) -> &StakingPoolGasConfig {
        &self.staking_pool
    }

    pub fn callbacks(&self) -> &CallBacksGasConfig {
        &self.callbacks
    }

    pub fn update(&mut self, config: &updates::GasConfig) {
        if let Some(staking_pool) = config.staking_pool.as_ref() {
            self.staking_pool.update(staking_pool);
        }

        if let Some(callbacks) = config.callbacks.as_ref() {
            self.callbacks.update(callbacks);
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct StakingPoolGasConfig {
    deposit_and_stake: u64,
    unstake: u64,
    withdraw: u64,
    get_account_balance: u64,
}

impl Default for StakingPoolGasConfig {
    fn default() -> Self {
        Self {
            deposit_and_stake: BASE_GAS * 3,
            unstake: BASE_GAS * 3,
            withdraw: BASE_GAS * 3,
            get_account_balance: BASE_GAS,
        }
    }
}

impl StakingPoolGasConfig {
    pub fn deposit_and_stake(&self) -> u64 {
        self.deposit_and_stake
    }

    pub fn unstake(&self) -> u64 {
        self.unstake
    }

    pub fn withdraw(&self) -> u64 {
        self.withdraw
    }

    pub fn get_account_balance(&self) -> u64 {
        self.get_account_balance
    }

    pub fn update(&mut self, config: &updates::StakingPoolGasConfig) {
        if let Some(deposit_and_stake) = config.deposit_and_stake {
            self.deposit_and_stake = deposit_and_stake;
        }

        if let Some(unstake) = config.unstake {
            self.unstake = unstake;
        }

        if let Some(withdraw) = config.withdraw {
            self.withdraw = withdraw;
        }

        if let Some(get_account_balance) = config.get_account_balance {
            self.get_account_balance = get_account_balance;
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CallBacksGasConfig {
    on_deposit_and_stake: u64,
}

impl CallBacksGasConfig {
    pub fn on_deposit_and_stake(&self) -> u64 {
        self.on_deposit_and_stake
    }

    pub fn update(&mut self, config: &updates::CallBacksGasConfig) {
        if let Some(on_deposit_and_stake) = config.on_deposit_and_stake {
            self.on_deposit_and_stake = on_deposit_and_stake;
        }
    }
}

impl Default for CallBacksGasConfig {
    fn default() -> Self {
        Self {
            on_deposit_and_stake: BASE_GAS,
        }
    }
}

/// provides support for config updates
/// - config updates can be uploaded in a serde compatible format (JSON and TOML are supported)
/// - all config properties are optional - thus only config properties that change need to be specified
///   when updating the config
pub mod updates {
    use near_sdk::serde::export::TryFrom;
    use near_sdk::{
        serde::{self, Deserialize, Serialize},
        serde_json,
    };
    use std::borrow::Borrow;

    #[derive(Debug)]
    pub struct ConfigParseError(String);

    impl AsRef<str> for ConfigParseError {
        fn as_ref(&self) -> &str {
            &self.0
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    #[serde(crate = "near_sdk::serde")]
    pub struct Config {
        /// TOML and JSON do not support u128 - thus, we need to encode u128 values as string
        pub storage_cost_per_byte: Option<String>,
        pub gas_config: Option<GasConfig>,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    #[serde(crate = "near_sdk::serde")]
    pub struct GasConfig {
        pub staking_pool: Option<StakingPoolGasConfig>,
        pub callbacks: Option<CallBacksGasConfig>,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    #[serde(crate = "near_sdk::serde")]
    pub struct StakingPoolGasConfig {
        pub deposit_and_stake: Option<u64>,
        pub unstake: Option<u64>,
        pub withdraw: Option<u64>,
        pub get_account_balance: Option<u64>,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    #[serde(crate = "near_sdk::serde")]
    pub struct CallBacksGasConfig {
        pub on_deposit_and_stake: Option<u64>,
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use std::str::FromStr;

    #[test]
    fn config_from_json() {
        let json_config: updates::Config = near_sdk::serde_json::from_str(
            r#"
        {
            "storage_cost_per_byte":"1000",
            "gas_config": {
                "staking_pool": {
                    "deposit_and_stake":500,
                    "get_account_balance":200
                },
                "callbacks": {
                    "on_deposit_and_stake":100
                }
            }
        }
        "#,
        )
        .unwrap();

        let mut config = Config::default();
        config.apply_updates(&json_config);
        assert_eq!(
            u128::from_str(&json_config.storage_cost_per_byte.unwrap()).unwrap(),
            1000u128
        );
        assert_eq!(
            json_config
                .gas_config
                .as_ref()
                .unwrap()
                .staking_pool
                .as_ref()
                .unwrap()
                .deposit_and_stake
                .unwrap(),
            500
        );
        assert_eq!(
            json_config
                .gas_config
                .as_ref()
                .unwrap()
                .staking_pool
                .as_ref()
                .unwrap()
                .get_account_balance
                .unwrap(),
            200
        );
        assert_eq!(
            json_config
                .gas_config
                .as_ref()
                .unwrap()
                .callbacks
                .as_ref()
                .unwrap()
                .on_deposit_and_stake
                .unwrap(),
            100
        );
    }
}
