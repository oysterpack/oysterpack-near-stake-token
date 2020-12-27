use crate::{
    domain::{Gas, YoctoNear, TGAS},
    interface,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
    storage_cost_per_byte: YoctoNear,
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
    pub fn new(storage_cost_per_byte: YoctoNear, gas_config: GasConfig) -> Self {
        Self {
            storage_cost_per_byte,
            gas_config,
        }
    }

    pub fn storage_cost_per_byte(&self) -> &YoctoNear {
        &self.storage_cost_per_byte
    }

    pub fn gas_config(&self) -> &GasConfig {
        &self.gas_config
    }

    pub fn update(&mut self, config: interface::Config) {
        if let Some(value) = config.storage_cost_per_byte {
            self.storage_cost_per_byte = value.value().into();
        }
        // TODO

        unimplemented!()
    }
}

/// Basic compute.
pub const GAS_BASE_COMPUTE: Gas = Gas(5_000_000_000_000);
/// Fee for function call promise.
pub const GAS_FOR_PROMISE: Gas = Gas(5_000_000_000_000);
/// Fee for the `.then` call.
pub const GAS_FOR_DATA_DEPENDENCY: Gas = Gas(10_000_000_000_000);

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct GasConfig {
    staking_pool: StakingPoolGasConfig,
    callbacks: CallBacksGasConfig,
    vault_ft: VaultFungibleTokenGasConfig,
}

impl GasConfig {
    pub fn staking_pool(&self) -> &StakingPoolGasConfig {
        &self.staking_pool
    }

    pub fn callbacks(&self) -> &CallBacksGasConfig {
        &self.callbacks
    }

    pub fn vault_fungible_token(&self) -> &VaultFungibleTokenGasConfig {
        &self.vault_ft
    }
}

impl Default for GasConfig {
    fn default() -> Self {
        Self {
            staking_pool: Default::default(),
            callbacks: Default::default(),
            vault_ft: Default::default(),
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
    get_account: Gas,
}

impl Default for StakingPoolGasConfig {
    fn default() -> Self {
        Self {
            get_account_balance: TGAS * 5,
            get_account: TGAS * 5,

            deposit_and_stake: TGAS * 50,
            unstake: TGAS * 50,
            withdraw: TGAS * 50,
        }
    }
}

impl StakingPoolGasConfig {
    pub fn deposit_and_stake(&self) -> Gas {
        self.deposit_and_stake
    }

    pub fn unstake(&self) -> Gas {
        TGAS * 50
        // self.unstake
    }

    pub fn withdraw(&self) -> Gas {
        TGAS * 50
        // self.withdraw
    }

    pub fn get_account_balance(&self) -> Gas {
        self.get_account_balance
    }

    pub fn get_account(&self) -> Gas {
        self.get_account
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct CallBacksGasConfig {
    on_run_stake_batch: Gas,
    on_deposit_and_stake: Gas,
    on_unstake: Gas,
    unlock: Gas,

    // used by redeem stake workflow
    on_run_redeem_stake_batch: Gas,
    on_redeeming_stake_pending_withdrawal: Gas,
    on_redeeming_stake_post_withdrawal: Gas,

    // used by ContractOwner interface
    finalize_ownership_transfer: Gas,
}

impl CallBacksGasConfig {
    pub fn on_deposit_and_stake(&self) -> Gas {
        self.on_deposit_and_stake
    }

    pub fn unlock(&self) -> Gas {
        self.unlock
    }

    pub fn on_run_stake_batch(&self) -> Gas {
        self.on_run_stake_batch
    }

    pub fn on_redeeming_stake_pending_withdrawal(&self) -> Gas {
        TGAS * 85
        // self.on_redeeming_stake_pending_withdrawal
    }

    pub fn on_redeeming_stake_post_withdrawal(&self) -> Gas {
        TGAS * 5
        // self.on_redeeming_stake_post_withdrawal
    }

    pub fn on_run_redeem_stake_batch(&self) -> Gas {
        TGAS * 85
        // self.on_run_redeem_stake_batch
    }

    pub fn on_unstake(&self) -> Gas {
        TGAS * 5
        // self.on_unstake
    }

    pub fn finalize_ownership_transfer(&self) -> Gas {
        self.finalize_ownership_transfer
    }
}

impl Default for CallBacksGasConfig {
    fn default() -> Self {
        Self {
            on_run_stake_batch: TGAS * 85,
            on_deposit_and_stake: TGAS * 5,
            unlock: TGAS * 5,

            on_run_redeem_stake_batch: TGAS * 85,
            on_unstake: TGAS * 5,

            on_redeeming_stake_pending_withdrawal: TGAS * 85,
            on_redeeming_stake_post_withdrawal: TGAS * 5,

            finalize_ownership_transfer: TGAS * 10,
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct VaultFungibleTokenGasConfig {
    min_gas_for_receiver: Gas,

    /// We need to create 2 promises with dependencies and with some basic compute to write to the state.
    transfer_with_vault: Gas,
    resolve_vault: Gas,
}

impl VaultFungibleTokenGasConfig {
    pub fn min_gas_for_receiver(&self) -> Gas {
        self.min_gas_for_receiver
    }

    pub fn resolve_vault(&self) -> Gas {
        self.resolve_vault
    }

    pub fn transfer_with_vault(&self) -> Gas {
        self.transfer_with_vault
    }
}

impl Default for VaultFungibleTokenGasConfig {
    fn default() -> Self {
        Self {
            min_gas_for_receiver: GAS_FOR_PROMISE + GAS_BASE_COMPUTE,
            transfer_with_vault: (GAS_FOR_PROMISE * 2) + GAS_FOR_DATA_DEPENDENCY + GAS_BASE_COMPUTE,
            resolve_vault: GAS_BASE_COMPUTE,
        }
    }
}
