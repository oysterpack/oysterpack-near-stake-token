use crate::near_env::Env;
use crate::{
    config::Config,
    near::{*},
    ContractSettings, StakeTokenContract,
};
use near_sdk::{
    serde::{Deserialize, Serialize},
    AccountId, PromiseResult, VMContext,
};


pub const EXPECTED_ACCOUNT_STORAGE_USAGE: u64 = 681;

pub fn expected_account_storage_fee() -> u128 {
    EXPECTED_ACCOUNT_STORAGE_USAGE as u128 * Config::default().storage_cost_per_byte().value()
}

pub fn default_contract_settings() -> ContractSettings {
    ContractSettings::new(
        "staking-pool.near".into(),
        "operator.stake.oysterpack.near".into(),
        None,
    )
    .unwrap()
}

pub fn stake_contract_account_id() -> AccountId {
    "stake.oysterpack.near".to_string()
}

pub fn new_context(predecessor_account_id: &str) -> VMContext {
    VMContext {
        current_account_id: stake_contract_account_id(),
        signer_account_id: predecessor_account_id.to_string(),
        signer_account_pk: vec![0, 1, 2],
        predecessor_account_id: predecessor_account_id.to_string(),
        input: vec![],
        epoch_height: 0,
        block_index: 0,
        block_timestamp: 0,
        account_balance: 100 * YOCTO,
        account_locked_balance: 0,
        storage_usage: 10u64.pow(6),
        attached_deposit: 0,
        prepaid_gas: 10u64.pow(18),
        random_seed: vec![0, 1, 2],
        is_view: false,
        output_data_receivers: vec![],
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Receipt {
    pub receiver_id: String,
    pub receipt_indices: Vec<usize>,
    pub actions: Vec<Action>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum Action {
    Transfer {
        deposit: u128,
    },
    FunctionCall {
        method_name: String,
        args: String,
        gas: u64,
        deposit: u128,
    },
}

pub fn set_env_with_success_promise_result(contract: &mut StakeTokenContract) {
    pub fn promise_result(_result_index: u64) -> PromiseResult {
        PromiseResult::Successful(vec![])
    }

    pub fn promise_results_count() -> u64 {
        1
    }

    contract.set_env(Env {
        promise_results_count_: promise_results_count,
        promise_result_: promise_result,
    });
}

pub fn set_env_with_failed_promise_result(contract: &mut StakeTokenContract) {
    pub fn promise_result(_result_index: u64) -> PromiseResult {
        PromiseResult::Failed
    }

    pub fn promise_results_count() -> u64 {
        1
    }

    contract.set_env(Env {
        promise_results_count_: promise_results_count,
        promise_result_: promise_result,
    });
}
