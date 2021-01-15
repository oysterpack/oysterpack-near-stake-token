use crate::interface::{AccountManagement, YoctoNear};
use crate::near_env::Env;
use crate::{near::*, ContractSettings, StakeTokenContract};
use near_sdk::test_utils::get_created_receipts;
use near_sdk::{
    serde::{Deserialize, Serialize},
    serde_json, testing_env, AccountId, MockedBlockchain, PromiseResult, VMContext,
};

pub const EXPECTED_ACCOUNT_STORAGE_USAGE: u64 = 681;

pub struct TestContext<'a> {
    pub contract: StakeTokenContract,
    pub account_id: &'a str,
    pub context: VMContext,
}

const ACCOUNT_ID: &str = "oysterpack.near";

impl<'a> TestContext<'a> {
    pub fn new(contract_settings: Option<ContractSettings>) -> Self {
        let mut context = new_context(ACCOUNT_ID);
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = contract_settings.unwrap_or_else(default_contract_settings);
        let contract = StakeTokenContract::new(None, contract_settings);

        Self {
            contract,
            account_id: ACCOUNT_ID,
            context,
        }
    }

    pub fn with_registered_account(contract_settings: Option<ContractSettings>) -> Self {
        let mut context = new_context(ACCOUNT_ID);
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = contract_settings.unwrap_or_else(default_contract_settings);
        let mut contract = StakeTokenContract::new(None, contract_settings);

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.register_account();
        context.account_balance += contract.account_storage_fee().value();

        context.attached_deposit = 0;
        testing_env!(context.clone());

        Self {
            contract,
            account_id: ACCOUNT_ID,
            context,
        }
    }
}

pub fn default_contract_settings() -> ContractSettings {
    ContractSettings::new(
        "staking-pool.near".into(),
        "operator.stake.oysterpack.near".into(),
        None,
    )
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
        account_balance: 10000 * YOCTO,
        account_locked_balance: 0,
        storage_usage: 400 * 1000,
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

pub fn deserialize_receipts() -> Vec<Receipt> {
    get_created_receipts()
        .iter()
        .map(|receipt| {
            let json = serde_json::to_string_pretty(receipt).unwrap();
            println!("{}", json);
            let receipt: Receipt = serde_json::from_str(&json).unwrap();
            receipt
        })
        .collect()
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeArgs {
    pub amount: near_sdk::json_types::U128,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct CheckStakeArgs {
    pub near_liquidity: Option<YoctoNear>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct OnDepositAndStakeArgs {
    pub near_liquidity: Option<YoctoNear>,
}
