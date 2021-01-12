#![allow(unused_imports)]

extern crate oysterpack_near_stake_token;

mod account_management_client;
mod operator_client;
mod test_utils;

use near_sdk::{
    serde_json::{self, json},
    PendingContractTx,
};
use near_sdk_sim::*;

use oysterpack_near_stake_token::{
    interface::{self, StakingService},
    near::NO_DEPOSIT,
};

use account_management_client::*;
use oysterpack_near_stake_token::domain::TGAS;
use oysterpack_near_stake_token::interface::contract_state::ContractState;
use oysterpack_near_stake_token::interface::Config;

lazy_static! {
    static ref WASM_BYTES: &'static [u8] =
        include_bytes!("../res/oysterpack_near_stake_token.wasm").as_ref();
}

#[test]
fn sim_test() {
    let ctx = test_utils::create_context();
    let contract = ctx.contract;
    let contract_account_id: &str = contract.user_account.account_id.as_str();
    let user = &ctx.contract_operator;

    let initial_contract_state = operator_client::contract_state(contract_account_id, user);
    check_contract_state_after_deployment(&initial_contract_state);

    let config = operator_client::config(contract_account_id, user);
    check_config_after_deployment(&config);
}

fn check_contract_state_after_deployment(contract_state: &ContractState) {
    println!("#############################################");
    println!("### check_contract_state_after_deployment ###");
    println!("{}", serde_json::to_string_pretty(contract_state).unwrap());
    assert_eq!(
        contract_state.storage_usage_growth.0 .0, 0,
        "after deployment the contract storage usage should be baselined at zero"
    );
    println!("=== check_contract_state_after_deployment === PASSED");
    println!("=====================================================")
}

fn check_config_after_deployment(config: &Config) {
    println!("#####################################");
    println!("### check_config_after_deployment ###");
    println!("{}", serde_json::to_string_pretty(config).unwrap());
    println!("=== check_config_after_deployment === PASSED");
    println!("============================================")
}
