use crate::{config::Config, near::*, ContractSettings, StakeTokenContract};
use near_sdk::{
    serde::{Deserialize, Serialize},
    testing_env, AccountId, MockedBlockchain, VMContext,
};

pub const EXPECTED_ACCOUNT_STORAGE_USAGE: u64 = 722;

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
}

pub fn expected_account_storage_fee() -> u128 {
    EXPECTED_ACCOUNT_STORAGE_USAGE as u128 * Config::default().storage_cost_per_byte().value()
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
        account_balance: 100 * YOCTO,
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
