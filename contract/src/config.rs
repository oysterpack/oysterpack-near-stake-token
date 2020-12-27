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
            self.gas_config.merge(gas_config);
        }
    }
}

/// Basic compute.
pub const GAS_BASE_COMPUTE: Gas = Gas(5_000_000_000_000);
/// Fee for function call promise.
pub const GAS_FOR_PROMISE: Gas = Gas(5_000_000_000_000);
/// Fee for the `.then` call.
pub const GAS_FOR_DATA_DEPENDENCY: Gas = Gas(10_000_000_000_000);

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy)]
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

    pub fn merge(&mut self, config: interface::GasConfig) {
        if let Some(config) = config.callbacks {
            self.callbacks.merge(config);
        }
        if let Some(config) = config.staking_pool {
            self.staking_pool.merge(config);
        }
        if let Some(config) = config.vault_ft {
            self.vault_ft.merge(config);
        }

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

impl Default for GasConfig {
    fn default() -> Self {
        Self {
            staking_pool: Default::default(),
            callbacks: Default::default(),
            vault_ft: Default::default(),
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

    pub fn merge(&mut self, config: interface::StakingPoolGasConfig) {
        if let Some(gas) = config.get_account_balance {
            assert!(
                gas >= TGAS * 5,
                "staking_pool::get_account_balance must be >= 5 TGas"
            );
            self.get_account_balance = gas;
        }
        if let Some(gas) = config.get_account {
            assert!(
                gas >= TGAS * 5,
                "staking_pool::get_account must be >= 5 TGas"
            );
            self.get_account = gas;
        }
        if let Some(gas) = config.deposit_and_stake {
            assert!(
                gas >= TGAS * 40,
                "staking_pool::deposit_and_stake must be >= 40 TGas"
            );
            self.deposit_and_stake = gas;
        }
        if let Some(gas) = config.unstake {
            assert!(gas >= TGAS * 40, "staking_pool::unstake must be >= 40 TGas");
            self.unstake = gas;
        }
        if let Some(gas) = config.withdraw {
            assert!(
                gas >= TGAS * 40,
                "staking_pool::withdraw must be >= 40 TGas"
            );
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

    // used by ContractOwner interface
    finalize_ownership_transfer: Gas,
}

impl CallBacksGasConfig {
    pub fn merge(&mut self, config: interface::CallBacksGasConfig) {
        if let Some(gas) = config.on_run_stake_batch {
            assert!(
                gas >= TGAS * 75,
                "callbacks::on_run_stake_batch must be >= 75 TGas"
            );
            self.on_run_stake_batch = gas;
        }
        if let Some(gas) = config.on_deposit_and_stake {
            assert!(
                gas >= TGAS * 5,
                "callbacks::on_deposit_and_stake must be >= 5 TGas"
            );
            self.on_deposit_and_stake = gas;
        }
        if let Some(gas) = config.on_unstake {
            assert!(gas >= TGAS * 5, "callbacks::on_unstake must be >= 5 TGas");
            self.on_unstake = gas;
        }
        if let Some(gas) = config.unlock {
            assert!(gas >= TGAS * 5, "callbacks::unlock must be >= 5 TGas");
            self.unlock = gas;
        }
        if let Some(gas) = config.on_run_redeem_stake_batch {
            assert!(
                gas >= TGAS * 75,
                "callbacks::on_run_redeem_stake_batch must be >= 75 TGas"
            );
            self.on_run_redeem_stake_batch = gas;
        }
        if let Some(gas) = config.on_redeeming_stake_pending_withdrawal {
            assert!(
                gas >= TGAS * 75,
                "callbacks::on_redeeming_stake_pending_withdrawal must be >= 75 TGas"
            );
            self.on_redeeming_stake_pending_withdrawal = gas;
        }
        if let Some(gas) = config.on_redeeming_stake_post_withdrawal {
            assert!(
                gas >= TGAS * 5,
                "callbacks::on_redeeming_stake_post_withdrawal must be >= 5 TGas"
            );
            self.on_redeeming_stake_post_withdrawal = gas;
        }
        if let Some(gas) = config.finalize_ownership_transfer {
            assert!(
                gas >= TGAS * 5,
                "callbacks::finalize_ownership_transfer must be >= 5 TGas"
            );
            self.finalize_ownership_transfer = gas;
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

#[derive(Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub struct VaultFungibleTokenGasConfig {
    min_gas_for_receiver: Gas,

    /// We need to create 2 promises with dependencies and with some basic compute to write to the state.
    transfer_with_vault: Gas,
    resolve_vault: Gas,
}

impl VaultFungibleTokenGasConfig {
    pub fn merge(&mut self, config: interface::VaultFungibleTokenGasConfig) {
        if let Some(gas) = config.min_gas_for_receiver {
            assert!(
                gas >= TGAS * 10,
                "vault_ft::min_gas_for_receiver must be >= 10 TGas"
            );
            self.min_gas_for_receiver = gas;
        }
        if let Some(gas) = config.transfer_with_vault {
            assert!(
                gas >= TGAS * 25,
                "vault_ft::transfer_with_vault must be >= 25 TGas"
            );
            self.transfer_with_vault = gas;
        }
        if let Some(gas) = config.resolve_vault {
            assert!(gas >= TGAS * 5, "vault_ft::resolve_vault must be >= 5 TGas");
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

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn vault_ft_gas_config_merge_success() {
        let mut config = VaultFungibleTokenGasConfig::default();
        config.merge(interface::VaultFungibleTokenGasConfig {
            min_gas_for_receiver: None,
            transfer_with_vault: None,
            resolve_vault: None,
        });

        config.merge(interface::VaultFungibleTokenGasConfig {
            min_gas_for_receiver: Some(TGAS * 100),
            transfer_with_vault: None,
            resolve_vault: None,
        });
        assert_eq!(config.min_gas_for_receiver, TGAS * 100);

        config.merge(interface::VaultFungibleTokenGasConfig {
            min_gas_for_receiver: Some(TGAS * 100),
            transfer_with_vault: Some(TGAS * 200),
            resolve_vault: None,
        });
        assert_eq!(config.min_gas_for_receiver, TGAS * 100);
        assert_eq!(config.transfer_with_vault, TGAS * 200);

        config.merge(interface::VaultFungibleTokenGasConfig {
            min_gas_for_receiver: Some(TGAS * 100),
            transfer_with_vault: Some(TGAS * 200),
            resolve_vault: Some(TGAS * 300),
        });
        assert_eq!(config.min_gas_for_receiver, TGAS * 100);
        assert_eq!(config.transfer_with_vault, TGAS * 200);
        assert_eq!(config.resolve_vault, TGAS * 300);
    }
}
