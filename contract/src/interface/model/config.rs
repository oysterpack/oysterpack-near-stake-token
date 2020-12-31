use crate::domain::{Gas, YoctoNear};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
    pub storage_cost_per_byte: Option<YoctoNear>,
    pub gas_config: Option<GasConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct GasConfig {
    pub staking_pool: Option<StakingPoolGasConfig>,
    pub callbacks: Option<CallBacksGasConfig>,
    pub vault_ft: Option<VaultFungibleTokenGasConfig>,
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct VaultFungibleTokenGasConfig {
    pub min_gas_for_receiver: Option<Gas>,
    pub transfer_with_vault: Option<Gas>,
    pub resolve_vault: Option<Gas>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct FungibleTokenTransferCallGasConfig {
    pub min_gas_for_receiver: Option<Gas>,
    pub transfer_call: Option<Gas>,
    pub finalize_ft_transfer: Option<Gas>,
}
