#![allow(dead_code)]

use crate::domain::TGAS;
use crate::interface::{AccountManagement, YoctoNear};
use crate::near_env::Env;
use crate::{near::*, StakeTokenContract};
use near_sdk::{
    json_types::ValidAccountId,
    serde::{Deserialize, Serialize},
    serde_json,
    test_utils::get_created_receipts,
    testing_env, AccountId, MockedBlockchain, PromiseResult, VMContext,
};
use std::convert::TryInto;
use std::ops::{Deref, DerefMut};

pub const EXPECTED_ACCOUNT_STORAGE_USAGE: u64 = 681;

pub struct TestContext<'a> {
    pub contract: StakeTokenContract,
    pub account_id: &'a str,
    pub context: VMContext,
}

pub const TEST_ACCOUNT_ID: &str = "oysterpack.near";

pub const TEST_STAKING_POOL_ID: &str = "staking-pool.near";
pub const TEST_OWNER_ID: &str = "owner.stake.oysterpack.near";
pub const TEST_OPERATOR_ID: &str = "operator.stake.oysterpack.near";

pub fn to_valid_account_id(account_id: &str) -> ValidAccountId {
    account_id.try_into().unwrap()
}

impl<'a> TestContext<'a> {
    pub fn with_vm_context(context: VMContext) -> Self {
        let mut context = context.clone();
        context.is_view = false;
        testing_env!(context.clone());

        let contract = StakeTokenContract::new(
            to_valid_account_id(TEST_STAKING_POOL_ID),
            to_valid_account_id(TEST_OWNER_ID),
            to_valid_account_id(TEST_OPERATOR_ID),
        );

        Self {
            contract,
            account_id: TEST_ACCOUNT_ID,
            context,
        }
    }

    pub fn new() -> Self {
        TestContext::with_vm_context(new_context(TEST_ACCOUNT_ID))
    }

    pub fn with_registered_account() -> Self {
        let mut context = new_context(TEST_ACCOUNT_ID);
        context.is_view = false;
        testing_env!(context.clone());

        let mut contract = StakeTokenContract::new(
            to_valid_account_id(TEST_STAKING_POOL_ID),
            to_valid_account_id(TEST_OWNER_ID),
            to_valid_account_id(TEST_OPERATOR_ID),
        );

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.register_account();
        context.account_balance += contract.account_storage_fee().value();

        context.attached_deposit = 0;
        context.storage_usage = contract.contract_initial_storage_usage.value();
        testing_env!(context.clone());

        Self {
            contract,
            account_id: TEST_ACCOUNT_ID,
            context,
        }
    }

    pub fn register_owner(&mut self) {
        self.register_account(TEST_OWNER_ID);
    }

    pub fn register_operator(&mut self) {
        self.register_account(TEST_OPERATOR_ID);
    }

    pub fn register_account(&mut self, account_id: &str) {
        let mut context = self.set_predecessor_account_id(account_id);
        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        self.contract.register_account();

        context.attached_deposit = 0;
        testing_env!(context);
    }

    pub fn set_predecessor_account_id(&mut self, account_id: &str) -> VMContext {
        let mut context = self.context.clone();
        context.predecessor_account_id = account_id.to_string();
        context
    }
}

impl<'a> Deref for TestContext<'a> {
    type Target = StakeTokenContract;

    fn deref(&self) -> &Self::Target {
        &self.contract
    }
}

impl<'a> DerefMut for TestContext<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.contract
    }
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
        prepaid_gas: (TGAS * 200).value(),
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

pub fn set_env_with_promise_result(
    contract: &mut StakeTokenContract,
    promise_result: fn(u64) -> PromiseResult,
) {
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
