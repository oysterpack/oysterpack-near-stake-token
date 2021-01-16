#![allow(unused_imports)]

//! before running the simulation test, make sure the wasm files are built for the STAKE token contrac
//! and the mock staking pool contract
//! ```shell
//! cd contract
//! ./build.sh
//!
//! cd staking-pool-mock
//! ./build.sh
//! ```

extern crate oysterpack_near_stake_token;

mod account_management_client;
mod financials_client;
mod operator_client;
mod staking_pool_client;
mod staking_service_client;
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
use oysterpack_near_stake_token::interface::{BatchId, Config, ContractBalances, YoctoNear};
use oysterpack_near_stake_token::interface::{StakeAccount, StakeBatch};
use oysterpack_near_stake_token::near::YOCTO;
use staking_service_client::*;
use std::convert::TryInto;
use test_utils::*;

#[test]
fn sim_test() {
    let ctx = test_utils::create_context();
    let user = &ctx.contract_operator;

    let (_initial_contract_state, _initial_config, _initial_contract_balances) =
        check_initial_state(&ctx, user);
    check_no_accounts_registered(&ctx, user);

    register_contract_owner_account(&ctx);
    register_user_accounts(&ctx);

    // simulates the entire work from depositing to unstaking and withdrawing
    deposit_funds_for_each_user_account(&ctx);
    stake(&ctx);
    check_user_accounts_after_deposits_are_staked(&ctx);

    redeem_all_stake_for_each_user_account(&ctx);
    check_user_accounts_after_redeeming_all_stake(&ctx);

    unstake(&ctx);
    check_pending_withdrawal(&ctx);
    check_user_accounts_after_redeemed_stake_is_unstaked(&ctx);

    unstake(&ctx); // while pending withdrawal
    check_pending_withdrawal(&ctx);
    unlock_funds_in_staking_pool(&ctx);

    unstake(&ctx); // unstaked NEAR should be withdrawn
    check_state_after_all_redeemed_and_withdrawn(&ctx);
    check_user_accounts_after_redeem_stake_batch_completed(&ctx);
}

fn check_user_accounts_after_redeem_stake_batch_completed(ctx: &TestContext) {
    println!("###############################################################");
    println!("### check_user_accounts_after_redeem_stake_batch_completed ####");

    unimplemented!();

    println!("=== check_user_accounts_after_redeem_stake_batch_completed ===");
    println!("==============================================================");
}

fn check_state_after_all_redeemed_and_withdrawn(ctx: &TestContext) {
    println!("#####################################################");
    println!("### check_state_after_all_redeemed_and_withdrawn ####");

    unimplemented!();

    println!("=== check_state_after_all_redeemed_and_withdrawn ===");
    println!("====================================================");
}

fn unlock_funds_in_staking_pool(ctx: &TestContext) {
    println!("#####################################");
    println!("### unlock_funds_in_staking_pool ####");

    unimplemented!();

    println!("=== unlock_funds_in_staking_pool ===");
    println!("====================================");
}

fn unstake(ctx: &TestContext) {
    println!("################");
    println!("### unstake ####");

    unimplemented!();

    println!("=== unstake ===");
    println!("===============");
}

fn check_user_accounts_after_redeemed_stake_is_unstaked(ctx: &TestContext) {
    println!("#############################################################");
    println!("### check_user_accounts_after_redeemed_stake_is_unstaked ####");

    unimplemented!();

    println!("=== check_user_accounts_after_redeemed_stake_is_unstaked ===");
    println!("============================================================");
}

fn check_pending_withdrawal(ctx: &TestContext) {
    println!("#################################");
    println!("### check_pending_withdrawal ####");

    unimplemented!();

    println!("=== check_pending_withdrawal ===");
    println!("================================");
}

fn check_user_accounts_after_redeeming_all_stake(ctx: &TestContext) {
    println!("######################################################");
    println!("### check_user_accounts_after_redeeming_all_stake ####");

    unimplemented!();

    println!("=== check_user_accounts_after_redeeming_all_stake ===");
    println!("=====================================================");
}

fn redeem_all_stake_for_each_user_account(ctx: &TestContext) {
    println!("###############################################");
    println!("### redeem_all_stake_for_each_user_account ####");

    unimplemented!();

    println!("=== redeem_all_stake_for_each_user_account ===");
    println!("==============================================");
}

fn check_user_accounts_after_deposits_are_staked(ctx: &TestContext) {
    println!("######################################################");
    println!("### check_user_accounts_after_deposits_are_staked ####");

    unimplemented!();

    println!("=== check_user_accounts_after_deposits_are_staked ===");
    println!("=====================================================");
}

fn deposit_funds_for_each_user_account(ctx: &TestContext) {
    println!("############################################");
    println!("### deposit_funds_for_each_user_account ####");

    let initial_contract_state = ctx.operator.contract_state(&ctx.master_account);
    let initial_batch_amount: YoctoNear = initial_contract_state
        .stake_batch
        .map_or(0.into(), |batch| batch.balance.amount);

    let mut amount = 0_u128;
    let mut total_deposit_amount = 0_u128;
    for user in ctx.users.values() {
        amount += 1;
        let deposit_amount: interface::YoctoNear = (YOCTO * amount).into();
        total_deposit_amount += deposit_amount.value();
        let batch_id: BatchId = ctx.staking_service.deposit(user, deposit_amount.clone());

        let stake_account: StakeAccount = ctx
            .account_management
            .lookup_account(&ctx.master_account, &user.account_id())
            .unwrap();
        let user_stake_batch = stake_account.stake_batch.unwrap();
        assert_eq!(user_stake_batch.id, batch_id);
        assert_eq!(user_stake_batch.balance.amount, deposit_amount);
    }
    println!("total_deposit_amount = {}", total_deposit_amount);

    // check that the StakeBatch amount matches
    let contract_state = ctx.operator.contract_state(&ctx.master_account);
    let batch: StakeBatch = contract_state.stake_batch.unwrap();
    assert_eq!(
        batch.balance.amount.value(),
        total_deposit_amount + initial_batch_amount.value()
    );

    println!("=== deposit_funds_for_each_user_account ===");
    println!("===========================================");
}

fn stake(ctx: &TestContext) {
    println!("##############");
    println!("### stake ####");

    let contract_state = ctx.operator.contract_state(&ctx.master_account);
    match contract_state.stake_batch {
        None => println!("there is no stake batch to stake"),
        Some(_) => {
            let result: ExecutionResult = ctx.staking_service.stake(&ctx.contract_operator);
            result.assert_success();

            ctx.runtime.borrow_mut().process_all().unwrap();
            ctx.runtime.borrow_mut().produce_blocks(10).unwrap();
            ctx.runtime.borrow_mut().process_all().unwrap();

            let contract_state: ContractState = ctx.operator.contract_state(&ctx.master_account);
            assert!(
                contract_state.stake_batch.is_none(),
                "stake batch should have been cleared"
            );
        }
    }

    println!("=== stake ===");
    println!("=============");
}

fn register_contract_owner_account(ctx: &TestContext) {
    println!("###########################################");
    println!("### register_account_for_contract_owner ###");

    let account_storage_fee = ctx
        .account_management
        .account_storage_fee(ctx.master_account());
    println!("account_storage_fee = {}", account_storage_fee);
    let gas = TGAS * 10;
    let result = ctx.account_management.register_account(
        ctx.contract_owner(),
        account_storage_fee.into(),
        gas,
    );
    result.assert_success();

    println!("=== register_account_for_contract_owner === PASSED");
    println!("==================================================");
}

fn register_user_accounts(ctx: &TestContext) {
    println!("##############################");
    println!("### register_user_accounts ###");

    let account_storage_fee = ctx
        .account_management
        .account_storage_fee(ctx.master_account());
    println!("account_storage_fee = {}", account_storage_fee);
    let gas = TGAS * 10;

    for user_account in ctx.users.values() {
        println!("registered user account: {}", user_account.account_id());
        let result =
            ctx.account_management
                .register_account(user_account, account_storage_fee.into(), gas);
        result.assert_success();
    }

    println!("=== register_user_accounts === PASSED");
    println!("=====================================");
}

fn check_initial_state(
    ctx: &TestContext,
    user: &UserAccount,
) -> (ContractState, Config, ContractBalances) {
    let initial_contract_state = ctx.operator.contract_state(user);
    check_contract_state_after_deployment(&initial_contract_state);

    let initial_config = ctx.operator.config(user);
    check_config_after_deployment(&initial_config);

    let initial_contract_balances = ctx.financials.balances(user);
    assert_eq!(initial_contract_balances, initial_contract_state.balances,
               "the balances returned via `contract_state()` should be the same as the balances retrieved directly");
    check_contract_balances_after_deployment(&initial_contract_balances);

    (
        initial_contract_state,
        initial_config,
        initial_contract_balances,
    )
}

fn check_no_accounts_registered(ctx: &TestContext, user: &UserAccount) {
    println!("####################################");
    println!("### check_no_accounts_registered ###");

    assert_eq!(ctx.account_management.total_registered_accounts(user), 0);
    assert!(ctx
        .account_management
        .lookup_account(user, &user.account_id())
        .is_none());

    println!("=== check_no_accounts_registered === PASSED");
    println!("===========================================")
}

fn check_contract_state_after_deployment(contract_state: &ContractState) {
    println!("#############################################");
    println!("### check_contract_state_after_deployment ###");

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
