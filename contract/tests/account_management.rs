extern crate oysterpack_near_stake_token;

mod test_utils;

use near_sdk::{serde_json::json, PendingContractTx};
use near_sdk_sim::*;

use oysterpack_near_stake_token::interface;

lazy_static! {
    static ref WASM_BYTES: &'static [u8] =
        include_bytes!("../res/oysterpack_near_stake_token.wasm").as_ref();
}

#[test]
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
}
