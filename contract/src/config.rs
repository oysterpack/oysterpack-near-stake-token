use crate::{
    domain::{Gas, YoctoNear, TGAS},
    interface,
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy)]
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

    /// ## Panics
    /// if validation fails
    pub fn merge(&mut self, config: interface::Config) {
        if let Some(storage_cost_per_byte) = config.storage_cost_per_byte {
            assert!(
                storage_cost_per_byte.value() > 0,
                "storage_cost_per_byte must be > 0"
            );
            self.storage_cost_per_byte = storage_cost_per_byte.value().into();
        }
        if let Some(gas_config) = config.gas_config {
            self.gas_config.merge(gas_config, true);
        }
    }

    /// performas no validation
    pub fn force_merge(&mut self, config: interface::Config) {
        if let Some(storage_cost_per_byte) = config.storage_cost_per_byte {
            self.storage_cost_per_byte = storage_cost_per_byte.value().into();
        }
        if let Some(gas_config) = config.gas_config {
            self.gas_config.merge(gas_config, false);
        }
    }
}

/// Basic compute.
pub const GAS_BASE_COMPUTE: Gas = Gas(5_000_000_000_000);
/// Fee for function call promise.
pub const GAS_FOR_PROMISE: Gas = Gas(5_000_000_000_000);
/// Fee for the `.then` call.
pub const GAS_FOR_DATA_DEPENDENCY: Gas = Gas(10_000_000_000_000);

fn assert_gas_range(gas: Gas, min: u8, max: u8, field: &str) {
    assert!(
        gas >= TGAS * min as u64 && gas <= TGAS * max as u64,
        "{} must be within {} - {} TGas",
        field,
        min,
        max
    );
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub struct GasConfig {
    staking_pool: StakingPoolGasConfig,
    callbacks: CallBacksGasConfig,
    vault_ft: VaultFungibleTokenGasConfig,
    transfer_call_ft: FungibleTokenTransferCallGasConfig,
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

    pub fn transfer_call_fungible_token(&self) -> &FungibleTokenTransferCallGasConfig {
        &self.transfer_call_ft
    }

    /// if validate is true, then merge performs some sanity checks on the config to
    /// catch mis-configurations.
    ///
    /// ## Panics
    /// if validation fails
    pub fn merge(&mut self, config: interface::GasConfig, validate: bool) {
        if let Some(config) = config.callbacks {
            self.callbacks.merge(config, validate);
        }
        if let Some(config) = config.staking_pool {
            self.staking_pool.merge(config, validate);
        }
        if let Some(config) = config.vault_ft {
            self.vault_ft.merge(config, validate);
        }

        if validate {
            // check that the numbers add up for cross-contract workflows
            assert!(
                self.callbacks.on_run_stake_batch
                    >= (self.staking_pool.deposit_and_stake
                        + self.callbacks.on_deposit_and_stake
                        + (TGAS * 5)),
                "callbacks.on_run_stake_batch must be >= \
            staking_pool.deposit_and_stake + callbacks.on_deposit_and_stake + 5 TGas"
            );
            assert!(
                self.callbacks.on_run_redeem_stake_batch
                    >= (self.staking_pool.unstake + self.callbacks.on_unstake + (TGAS * 5)),
                "callbacks.on_run_redeem_stake_batch must be >= \
            staking_pool.unstake + callbacks.on_unstake + 5 TGas"
            );
            assert!(
                self.callbacks.on_redeeming_stake_pending_withdrawal
                    >= (self.staking_pool.withdraw
                        + self.callbacks.on_redeeming_stake_post_withdrawal
                        + (TGAS * 5)),
                "callbacks.on_redeeming_stake_pending_withdrawal must be >= \
            staking_pool.withdraw + callbacks.on_redeeming_stake_post_withdrawal + 5 TGas"
            );
        }
    }
}

impl Default for GasConfig {
    fn default() -> Self {
        Self {
            staking_pool: Default::default(),
            callbacks: Default::default(),
            vault_ft: Default::default(),
            transfer_call_ft: Default::default(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy)]
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
        self.unstake
    }

    pub fn withdraw(&self) -> Gas {
        self.withdraw
    }

    pub fn get_account_balance(&self) -> Gas {
        self.get_account_balance
    }

    pub fn get_account(&self) -> Gas {
        self.get_account
    }

    pub fn merge(&mut self, config: interface::StakingPoolGasConfig, validate: bool) {
        if let Some(gas) = config.get_account_balance {
            if validate {
                assert_gas_range(gas, 5, 10, "staking_pool::get_account_balance");
            }
            self.get_account_balance = gas;
        }
        if let Some(gas) = config.get_account {
            if validate {
                assert_gas_range(gas, 5, 10, "staking_pool::get_account");
            }
            self.get_account = gas;
        }
        if let Some(gas) = config.deposit_and_stake {
            if validate {
                assert_gas_range(gas, 40, 75, "staking_pool::deposit_and_stake");
            }
            self.deposit_and_stake = gas;
        }
        if let Some(gas) = config.unstake {
            if validate {
                assert_gas_range(gas, 40, 75, "staking_pool::unstake");
            }
            self.unstake = gas;
        }
        if let Some(gas) = config.withdraw {
            if validate {
                assert_gas_range(gas, 40, 75, "staking_pool::withdraw");
            }
            self.withdraw = gas;
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy)]
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
}

impl CallBacksGasConfig {
    pub fn merge(&mut self, config: interface::CallBacksGasConfig, validate: bool) {
        if let Some(gas) = config.on_run_stake_batch {
            if validate {
                assert_gas_range(gas, 70, 100, "callbacks::on_run_stake_batch");
            }
            self.on_run_stake_batch = gas;
        }
        if let Some(gas) = config.on_deposit_and_stake {
            if validate {
                assert_gas_range(gas, 5, 10, "callbacks::on_deposit_and_stake");
            }
            self.on_deposit_and_stake = gas;
        }
        if let Some(gas) = config.on_unstake {
            if validate {
                assert_gas_range(gas, 5, 10, "callbacks::on_unstake");
            }
            self.on_unstake = gas;
        }
        if let Some(gas) = config.unlock {
            if validate {
                assert_gas_range(gas, 5, 10, "callbacks::unlock");
            }
            self.unlock = gas;
        }
        if let Some(gas) = config.on_run_redeem_stake_batch {
            if validate {
                assert_gas_range(gas, 70, 100, "callbacks::on_run_redeem_stake_batch");
            }
            self.on_run_redeem_stake_batch = gas;
        }
        if let Some(gas) = config.on_redeeming_stake_pending_withdrawal {
            if validate {
                assert_gas_range(
                    gas,
                    70,
                    100,
                    "callbacks::on_redeeming_stake_pending_withdrawal",
                );
            }
            self.on_redeeming_stake_pending_withdrawal = gas;
        }
        if let Some(gas) = config.on_redeeming_stake_post_withdrawal {
            if validate {
                assert_gas_range(gas, 5, 10, "callbacks::on_redeeming_stake_post_withdrawal");
            }
            self.on_redeeming_stake_post_withdrawal = gas;
        }
    }

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
        self.on_redeeming_stake_pending_withdrawal
    }

    pub fn on_redeeming_stake_post_withdrawal(&self) -> Gas {
        self.on_redeeming_stake_post_withdrawal
    }

    pub fn on_run_redeem_stake_batch(&self) -> Gas {
        self.on_run_redeem_stake_batch
    }

    pub fn on_unstake(&self) -> Gas {
        self.on_unstake
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
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub struct VaultFungibleTokenGasConfig {
    min_gas_for_receiver: Gas,

    /// We need to create 2 promises with dependencies and with some basic compute to write to the state.
    transfer_with_vault: Gas,
    resolve_vault: Gas,
}

impl VaultFungibleTokenGasConfig {
    pub fn merge(&mut self, config: interface::VaultFungibleTokenGasConfig, validate: bool) {
        if let Some(gas) = config.min_gas_for_receiver {
            if validate {
                assert_gas_range(gas, 10, 20, "vault_ft::min_gas_for_receiver");
            }
            self.min_gas_for_receiver = gas;
        }
        if let Some(gas) = config.transfer_with_vault {
            if validate {
                assert_gas_range(gas, 20, 30, "vault_ft::transfer_with_vault");
            }
            self.transfer_with_vault = gas;
        }
        if let Some(gas) = config.resolve_vault {
            if validate {
                assert_gas_range(gas, 5, 10, "vault_ft::resolve_vault");
            }
            self.resolve_vault = gas;
        }
    }

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

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub struct FungibleTokenTransferCallGasConfig {
    min_gas_for_receiver: Gas,

    /// We need to create 2 promises with dependencies and with some basic compute to write to the state.
    transfer_call: Gas,
    finalize_ft_transfer: Gas,
}

impl FungibleTokenTransferCallGasConfig {
    pub fn merge(&mut self, config: interface::FungibleTokenTransferCallGasConfig, validate: bool) {
        if let Some(gas) = config.min_gas_for_receiver {
            if validate {
                assert_gas_range(gas, 10, 20, "transfer_call::min_gas_for_receiver");
            }
            self.min_gas_for_receiver = gas;
        }
        if let Some(gas) = config.transfer_call {
            if validate {
                assert_gas_range(gas, 20, 30, "transfer_call::transfer_call");
            }
            self.transfer_call = gas;
        }
        if let Some(gas) = config.finalize_ft_transfer {
            if validate {
                assert_gas_range(gas, 5, 10, "transfer_call::finalize_ft_transfer");
            }
            self.finalize_ft_transfer = gas;
        }
    }

    pub fn min_gas_for_receiver(&self) -> Gas {
        self.min_gas_for_receiver
    }

    pub fn transfer_call(&self) -> Gas {
        self.transfer_call
    }

    pub fn finalize_ft_transfer(&self) -> Gas {
        self.finalize_ft_transfer
    }
}

impl Default for FungibleTokenTransferCallGasConfig {
    fn default() -> Self {
        Self {
            min_gas_for_receiver: GAS_BASE_COMPUTE,
            transfer_call: (GAS_FOR_PROMISE * 2) + GAS_FOR_DATA_DEPENDENCY + GAS_BASE_COMPUTE,
            finalize_ft_transfer: GAS_BASE_COMPUTE,
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn vault_ft_gas_config_merge_success() {
        let mut config = VaultFungibleTokenGasConfig::default();

        config.merge(
            interface::VaultFungibleTokenGasConfig {
                min_gas_for_receiver: Some(TGAS * 11),
                transfer_with_vault: Some(TGAS * 21),
                resolve_vault: Some(TGAS * 6),
            },
            true,
        );
        assert_eq!(config.min_gas_for_receiver, TGAS * 11);
        assert_eq!(config.transfer_with_vault, TGAS * 21);
        assert_eq!(config.resolve_vault, TGAS * 6);
    }

    #[test]
    fn callbacks_gas_config_merge_success() {
        let mut config = CallBacksGasConfig::default();
        config.merge(
            interface::CallBacksGasConfig {
                on_run_stake_batch: Some(TGAS * 71),
                on_deposit_and_stake: Some(TGAS * 6),
                on_unstake: Some(TGAS * 7),
                unlock: Some(TGAS * 8),
                on_run_redeem_stake_batch: Some(TGAS * 72),
                on_redeeming_stake_pending_withdrawal: Some(TGAS * 73),
                on_redeeming_stake_post_withdrawal: Some(TGAS * 9),
            },
            true,
        );
        assert_eq!(config.on_run_stake_batch, TGAS * 71);
        assert_eq!(config.on_deposit_and_stake, TGAS * 6);
        assert_eq!(config.on_unstake, TGAS * 7);
        assert_eq!(config.unlock, TGAS * 8);
        assert_eq!(config.on_run_redeem_stake_batch, TGAS * 72);
        assert_eq!(config.on_redeeming_stake_pending_withdrawal, TGAS * 73);
        assert_eq!(config.on_redeeming_stake_post_withdrawal, TGAS * 9);
    }

    #[test]
    fn staking_pool_gas_config_merge_success() {
        let mut config = StakingPoolGasConfig::default();
        config.merge(
            interface::StakingPoolGasConfig {
                deposit_and_stake: Some(TGAS * 71),
                unstake: Some(TGAS * 72),
                withdraw: Some(TGAS * 73),
                get_account_balance: Some(TGAS * 6),
                get_account: Some(TGAS * 7),
            },
            true,
        );
        assert_eq!(config.deposit_and_stake, TGAS * 71);
        assert_eq!(config.unstake, TGAS * 72);
        assert_eq!(config.withdraw, TGAS * 73);
        assert_eq!(config.get_account_balance, TGAS * 6);
        assert_eq!(config.get_account, TGAS * 7);
    }
}
