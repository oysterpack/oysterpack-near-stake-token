extern crate oysterpack_near_stake_token;

mod account_management_client;
mod test_utils;

use near_sdk::{serde_json::json, PendingContractTx};
use near_sdk_sim::*;

lazy_static! {
    static ref WASM_BYTES: &'static [u8] =
        include_bytes!("../res/oysterpack_near_stake_token.wasm").as_ref();
}

/// https://github.com/near/near-sdk-rs/issues/262
#[test]
fn near_sdk_issue_262_near_bindgen_does_not_detect_pub_visibility_from_trait() {
    let ctx = test_utils::create_context();
    let contract = ctx.contract;

    // works because public method is defined directly on the contract
    let res = view!(contract.get_staking_pool_id());
    assert!(res.is_ok());

    // `staking_pool_id` is defined on the `StakingService` trait which the contract implements
    // but the method is not publicly visible on `StakeTokenContractContract`
    // let res = view!(contract.staking_pool_id());  // will not compile

    // but it is there if invoked via the "low level" approach:
    let res = ctx.contract_operator.view(PendingContractTx::new(
        &contract.user_account.account_id,
        "staking_pool_id",
        json!({}),
        true,
    ));

    assert!(res.is_ok());
    let staking_pool_id: String = res.unwrap_json();
    assert_eq!(staking_pool_id, "astro-stakers.poolv1.near");
}
