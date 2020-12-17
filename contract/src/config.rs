use crate::domain::{Gas, YoctoNearValue};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
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

pub const BASE_GAS: Gas = Gas(25_000_000_000_000);

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
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

impl Default for GasConfig {
    fn default() -> Self {
        Self {
            staking_pool: Default::default(),
            callbacks: Default::default(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolGasConfig {
    deposit_and_stake: Gas,
    unstake: Gas,
    withdraw: Gas,
    get_account_balance: Gas,
    is_account_unstaked_balance_available: Gas,
}

impl Default for StakingPoolGasConfig {
    fn default() -> Self {
        Self {
            deposit_and_stake: (BASE_GAS.value() * 3).into(),
            unstake: (BASE_GAS.value() * 3).into(),
            withdraw: (BASE_GAS.value() * 3).into(),
            get_account_balance: BASE_GAS,
            is_account_unstaked_balance_available: BASE_GAS,
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

    pub fn is_account_unstaked_balance_available(&self) -> Gas {
        self.is_account_unstaked_balance_available
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct CallBacksGasConfig {
    on_run_stake_batch: Gas,
    on_deposit_and_stake: Gas,
    on_get_account_staked_balance: Gas,
    unlock: Gas,

    // used by redeem stake workflow
    on_checking_staking_pool_for_fund_withdrawal_availability: Gas,
}

impl CallBacksGasConfig {
    pub fn on_deposit_and_stake(&self) -> Gas {
        self.on_deposit_and_stake
    }

    pub fn on_get_account_staked_balance(&self) -> Gas {
        self.on_get_account_staked_balance
    }

    pub fn unlock(&self) -> Gas {
        self.unlock
    }

    pub fn on_run_stake_batch(&self) -> Gas {
        self.on_run_stake_batch
    }

    pub fn on_checking_staking_pool_for_fund_withdrawal_availability(&self) -> Gas {
        self.on_checking_staking_pool_for_fund_withdrawal_availability
    }
}

impl Default for CallBacksGasConfig {
    fn default() -> Self {
        Self {
            on_run_stake_batch: (BASE_GAS.value() * 3).into(),
            on_deposit_and_stake: (BASE_GAS.value() * 3).into(),
            on_get_account_staked_balance: (BASE_GAS.value() * 3).into(),
            unlock: (BASE_GAS.value() * 3).into(),
            on_checking_staking_pool_for_fund_withdrawal_availability: (BASE_GAS.value() * 3)
                .into(),
        }
    }
}
