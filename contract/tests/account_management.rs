extern crate oysterpack_near_stake_token;

mod test_utils;

use near_sdk::{serde_json::json, PendingContractTx};
use near_sdk_sim::*;

use oysterpack_near_stake_token::{interface, near::NO_DEPOSIT};

lazy_static! {
    static ref WASM_BYTES: &'static [u8] =
        include_bytes!("../res/oysterpack_near_stake_token.wasm").as_ref();
}

#[test]
#[ignore]
fn account_management_tests() {
    let ctx = test_utils::create_context();
    let contract = ctx.contract;

    let res = ctx.contract_operator.view(PendingContractTx::new(
        &contract.user_account.account_id,
        "owner_balance",
        json!({}),
        true,
    ));

    assert!(res.is_ok());

    let msg: interface::YoctoNear = res.unwrap_json();
    println!("{:?}", msg);

    let result = ctx.contract_operator.view(PendingContractTx::new(
        &contract.user_account.account_id,
        "staking_pool_id",
        json!({}),
        true,
    ));
    let staking_pool_id: String = result.unwrap_json();
    println!("{}", staking_pool_id);

    let result = ctx.contract_operator.call(
        PendingContractTx::new(
            &contract.user_account.account_id,
            "on_run_redeem_stake_batch",
            json!({
            "staking_pool_account" : {
                "account_id": "account.near",
                "unstaked_balance": "0",
                "staked_balance": "0",
                "can_withdraw": false,
            }
            }),
            false,
        ),
        NO_DEPOSIT.value(),
        DEFAULT_GAS,
    );
    println!("{:?}", result);
    test_utils::assert_private_func_call(result, "on_run_redeem_stake_batch");
}
