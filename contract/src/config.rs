use crate::domain::{Gas, YoctoNearValue};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
    storage_cost_per_byte: YoctoNearValue,
    gas_config: GasConfig,
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

impl Config {
    pub fn new(storage_cost_per_byte: YoctoNearValue, gas_config: GasConfig) -> Self {
        Self {
            storage_cost_per_byte,
            gas_config,
        }
    }

    pub fn storage_cost_per_byte(&self) -> &YoctoNearValue {
        &self.storage_cost_per_byte
    }

    pub fn gas_config(&self) -> &GasConfig {
        &self.gas_config
    }
}

const BASE_GAS: Gas = Gas(25_000_000_000_000);

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Default)]
#[serde(crate = "near_sdk::serde")]
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
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolGasConfig {
    deposit_and_stake: Gas,
    unstake: Gas,
    withdraw: Gas,
    get_account_balance: Gas,
}

impl Default for StakingPoolGasConfig {
    fn default() -> Self {
        Self {
            deposit_and_stake: (BASE_GAS.value() * 3).into(),
            unstake: (BASE_GAS.value() * 3).into(),
            withdraw: (BASE_GAS.value() * 3).into(),
            get_account_balance: BASE_GAS,
        }
    }
}

impl StakingPoolGasConfig {
    pub fn deposit_and_stake(&self) -> Gas {
        self.deposit_and_stake
    }

    pub fn unstake(&self) -> Gas {
        self.unstake
    }

    pub fn withdraw(&self) -> Gas {
        self.withdraw
    }

    pub fn get_account_balance(&self) -> Gas {
        self.get_account_balance
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct CallBacksGasConfig {
    on_deposit_and_stake: Gas,
}

impl CallBacksGasConfig {
    pub fn on_deposit_and_stake(&self) -> Gas {
        self.on_deposit_and_stake
    }
}

impl Default for CallBacksGasConfig {
    fn default() -> Self {
        Self {
            on_deposit_and_stake: BASE_GAS,
        }
    }
}
