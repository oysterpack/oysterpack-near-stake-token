#![allow(unused_imports)]

extern crate oysterpack_near_stake_token;

mod account_management_client;
mod financials_client;
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
use financials_client::*;
use oysterpack_near_stake_token::config::CONTRACT_MIN_OPERATIONAL_BALANCE;
use oysterpack_near_stake_token::domain::TGAS;
use oysterpack_near_stake_token::interface::contract_state::ContractState;
use oysterpack_near_stake_token::interface::{Config, ContractBalances};
use test_utils::*;

lazy_static! {
    static ref WASM_BYTES: &'static [u8] =
        include_bytes!("../res/oysterpack_near_stake_token.wasm").as_ref();
}

#[test]
fn sim_test() {
    let ctx = test_utils::create_context();
    let contract = ctx.contract();
    let contract_account_id: &str = contract.user_account.account_id.as_str();
    let user = &ctx.contract_operator;

    let (initial_contract_state, initial_config, initial_contract_balances) =
        check_initial_state(contract_account_id, user);
    check_no_accounts_registered(contract_account_id, user);

    register_account_for_contract_owner(&ctx);
}

fn register_account_for_contract_owner(ctx: &TestContext) {
    let account_storage_fee = account_management_client::account_storage_fee(
        ctx.contract_account_id(),
        ctx.master_account(),
    );
    let gas = TGAS * 10;
    let result = account_management_client::register_account(
        ctx.contract_account_id(),
        ctx.contract_owner(),
        account_storage_fee.into(),
        gas,
    );
    result.assert_success();
}

fn check_initial_state(
    contract_account_id: &str,
    user: &UserAccount,
) -> (ContractState, Config, ContractBalances) {
    let initial_contract_state = operator_client::contract_state(contract_account_id, user);
    check_contract_state_after_deployment(&initial_contract_state);

    let initial_config = operator_client::config(contract_account_id, user);
    check_config_after_deployment(&initial_config);

    let initial_contract_balances = balances(contract_account_id, user);
    assert_eq!(initial_contract_balances, initial_contract_state.balances,
               "the balances returned via `contract_state()` should be the same as the balances retrieved directly");
    check_contract_balances_after_deployment(&initial_contract_balances);

    (
        initial_contract_state,
        initial_config,
        initial_contract_balances,
    )
}

fn check_no_accounts_registered(contract_account_id: &str, user: &UserAccount) {
    assert_eq!(
        account_management_client::total_registered_accounts(contract_account_id, user),
        0
    );
    assert!(
        account_management_client::lookup_account(contract_account_id, user, &user.account_id)
            .is_none()
    );
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
    // TODO

    println!("=== check_config_after_deployment === PASSED");
    println!("============================================")
}

fn check_contract_balances_after_deployment(balances: &ContractBalances) {
    println!("#####################################");
    println!("### check_contract_balances_after_deployment ###");

    println!("{}", serde_json::to_string_pretty(balances).unwrap());
    assert_eq!(
        balances.total_contract_storage_usage_cost.value()
            + balances.total_available_balance.value(),
        balances.total_contract_balance.value(),
        "total available balance = total contract balance minus contract's storage usage cost"
    );
    assert_eq!(
        balances.contract_required_operational_balance.value(),
        CONTRACT_MIN_OPERATIONAL_BALANCE.value(),
        "contract min operational balance did not match"
    );
    assert_eq!(
        balances.contract_owner_available_balance.value(),
        balances.total_available_balance.value() - CONTRACT_MIN_OPERATIONAL_BALANCE.value(),
        "contract owner available balance should be the entire contract available balance minus the min operational balance"
    );

    println!("=== check_contract_balances_after_deployment === PASSED");
    println!("=======================================================")
}
