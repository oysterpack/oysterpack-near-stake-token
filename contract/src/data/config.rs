use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

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
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CallBacksGasConfig {
    on_deposit_and_stake: u64,
}

impl CallBacksGasConfig {
    pub fn on_deposit_and_stake(&self) -> u64 {
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
