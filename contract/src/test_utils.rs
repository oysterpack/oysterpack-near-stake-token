#![allow(dead_code)]

use crate::interface::AccountManagement;
use crate::near_env::Env;
use crate::{near::*, Contract};
use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{
    json_types::ValidAccountId,
    serde::{Deserialize, Serialize},
    serde_json,
    test_utils::get_created_receipts,
    testing_env, MockedBlockchain, PromiseResult, VMContext,
};
use std::convert::TryInto;
use std::ops::{Deref, DerefMut};

pub struct TestContext<'a> {
    pub contract: Contract,
    pub account_id: &'a str,
    pub context: VMContext,
}

pub fn to_valid_account_id(account_id: &str) -> ValidAccountId {
    account_id.try_into().unwrap()
}

const TEST_ACCOUNT_ID: &str = "oysterpack.near";
const TEST_STAKING_POOL_ID: &str = "staking-pool.near";
pub const TEST_OWNER_ID: &str = "owner.stake.oysterpack.near";
pub const TEST_OPERATOR_ID: &str = "operator.stake.oysterpack.near";

impl<'a> TestContext<'a> {
    pub fn with_vm_context(context: VMContext) -> Self {
        let mut context = context.clone();
        context.is_view = false;
        testing_env!(context.clone());

        let contract = Contract::new(
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

    /// uses [`TEST_ACCOUNT_ID`] as the predecessor account ID
    pub fn new() -> Self {
        TestContext::with_vm_context(new_context(TEST_ACCOUNT_ID))
    }

    /// uses [`TEST_ACCOUNT_ID`] as the predecessor account ID and registers the account with the contract
    pub fn with_registered_account() -> Self {
        let mut context = new_context(TEST_ACCOUNT_ID);
        testing_env!(context.clone());

        let mut contract = Contract::new(
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
    type Target = Contract;

    fn deref(&self) -> &Self::Target {
        &self.contract
    }
}

impl<'a> DerefMut for TestContext<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.contract
    }
}

pub fn new_context(predecessor_account_id: &str) -> VMContext {
    VMContextBuilder::new()
        .current_account_id("stake.oysterpack.near".to_string())
        .signer_account_id(predecessor_account_id.to_string())
        .predecessor_account_id(predecessor_account_id.to_string())
        .account_balance(10000 * YOCTO)
        .build()
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

pub fn set_env_with_success_promise_result(contract: &mut Contract) {
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
    contract: &mut Contract,
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

pub fn set_env_with_failed_promise_result(contract: &mut Contract) {
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
