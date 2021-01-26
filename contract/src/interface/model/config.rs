use crate::{
    config,
    interface::{Gas, YoctoNear},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
    pub storage_cost_per_byte: Option<YoctoNear>,
    pub gas_config: Option<GasConfig>,
    /// percentage of contract gas rewards that are distributed to the contract owner
    /// - the rest of the contract earnings are staked to boost the staking rewards for user accounts
    /// - must be a number between 0-100
    pub contract_owner_earnings_percentage: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct GasConfig {
    pub staking_pool: Option<StakingPoolGasConfig>,
    pub callbacks: Option<CallBacksGasConfig>,

    pub function_call_promise: Option<Gas>,
    pub function_call_promise_data_dependency: Option<Gas>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolGasConfig {
    pub deposit_and_stake: Option<Gas>,
    pub deposit: Option<Gas>,
    pub stake: Option<Gas>,
    pub unstake: Option<Gas>,
    pub withdraw: Option<Gas>,
    pub get_account: Option<Gas>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct CallBacksGasConfig {
    pub on_run_stake_batch: Option<Gas>,
    pub on_deposit_and_stake: Option<Gas>,
    pub on_unstake: Option<Gas>,
    pub unlock: Option<Gas>,

    // used by redeem stake workflow
    pub on_run_redeem_stake_batch: Option<Gas>,
    pub on_redeeming_stake_pending_withdrawal: Option<Gas>,
    pub on_redeeming_stake_post_withdrawal: Option<Gas>,

    /// used by FungibleToken transfer call workflow
    pub resolve_transfer_gas: Option<Gas>,
}

impl From<config::Config> for Config {
    fn from(value: config::Config) -> Self {
        Self {
            storage_cost_per_byte: Some(value.storage_cost_per_byte().into()),
            gas_config: Some(value.gas_config().into()),
            contract_owner_earnings_percentage: Some(value.contract_owner_earnings_percentage()),
        }
    }
}

impl From<config::GasConfig> for GasConfig {
    fn from(value: config::GasConfig) -> Self {
        Self {
            staking_pool: Some(value.staking_pool().into()),
            callbacks: Some(value.callbacks().into()),
            function_call_promise: Some(value.function_call_promise().into()),
            function_call_promise_data_dependency: Some(
                value.function_call_promise_data_dependency().into(),
            ),
        }
    }
}

impl From<config::StakingPoolGasConfig> for StakingPoolGasConfig {
    fn from(value: config::StakingPoolGasConfig) -> Self {
        Self {
            deposit_and_stake: Some(value.deposit_and_stake().into()),
            deposit: Some(value.deposit().into()),
            stake: Some(value.stake().into()),
            unstake: Some(value.unstake().into()),
            withdraw: Some(value.withdraw().into()),
            get_account: Some(value.get_account().into()),
        }
    }
}

impl From<config::CallBacksGasConfig> for CallBacksGasConfig {
    fn from(value: config::CallBacksGasConfig) -> Self {
        Self {
            on_run_stake_batch: Some(value.on_run_stake_batch().into()),
            on_deposit_and_stake: Some(value.on_deposit_and_stake().into()),
            on_unstake: Some(value.on_unstake().into()),
            unlock: Some(value.unlock().into()),
            on_run_redeem_stake_batch: Some(value.on_run_redeem_stake_batch().into()),
            on_redeeming_stake_pending_withdrawal: Some(
                value.on_redeeming_stake_pending_withdrawal().into(),
            ),
            on_redeeming_stake_post_withdrawal: Some(
                value.on_redeeming_stake_post_withdrawal().into(),
            ),
            resolve_transfer_gas: Some(value.resolve_transfer_gas().into()),
        }
    }
}
