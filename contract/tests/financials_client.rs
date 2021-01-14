#![allow(dead_code)]

use near_sdk::{serde_json::json, PendingContractTx};
use near_sdk_sim::*;
use oysterpack_near_stake_token::{
    domain::{Gas, YoctoNear},
    interface::{Config, ContractBalances},
};

pub fn balances(contract_account_id: &str, user: &UserAccount) -> ContractBalances {
    let result = user.view(PendingContractTx::new(
        contract_account_id,
        "balances",
        json!({}),
        true,
    ));

    result.unwrap_json()
}

pub fn deposit_earnings(
    contract_account_id: &str,
    user: &UserAccount,
    deposit: YoctoNear,
    gas: Gas,
) -> ContractBalances {
    let result = user.call(
        PendingContractTx::new(contract_account_id, "deposit_earnings", json!({}), false),
        deposit.value(),
        gas.value(),
    );

    result.unwrap_json()
}
