use near_sdk::json_types::U128;
use near_sdk::{serde_json::json, PendingContractTx};
use near_sdk_sim::*;
use oysterpack_near_stake_token::domain::{Gas, YoctoNear};
use oysterpack_near_stake_token::interface::StakeAccount;
use oysterpack_near_stake_token::near::NO_DEPOSIT;

pub fn register_account(
    contract_account_id: &str,
    user: &UserAccount,
    deposit: YoctoNear,
    gas: Gas,
) -> ExecutionResult {
    user.call(
        PendingContractTx::new(contract_account_id, "register_account", json!({}), false),
        deposit.value(),
        gas.value(),
    )
}

pub fn unregister_account(
    contract_account_id: &str,
    user: &UserAccount,
    gas: Gas,
) -> ExecutionResult {
    user.call(
        PendingContractTx::new(contract_account_id, "unregister_account", json!({}), false),
        NO_DEPOSIT.value(),
        gas.value(),
    )
}

pub fn account_storage_fee(contract_account_id: &str, user: &UserAccount) -> YoctoNear {
    let result = user.view(PendingContractTx::new(
        contract_account_id,
        "account_storage_fee",
        json!({}),
        true,
    ));

    let count: U128 = result.unwrap_json();
    count.0.into()
}

pub fn account_registered(contract_account_id: &str, user: &UserAccount, account_id: &str) -> bool {
    let result = user.view(PendingContractTx::new(
        contract_account_id,
        "account_registered",
        json!({ "account_id": account_id }),
        true,
    ));

    result.unwrap_json()
}

pub fn total_registered_accounts(contract_account_id: &str, user: &UserAccount) -> u128 {
    let result = user.view(PendingContractTx::new(
        contract_account_id,
        "total_registered_accounts",
        json!({}),
        true,
    ));

    let count: U128 = result.unwrap_json();
    count.into()
}

pub fn lookup_account(
    contract_account_id: &str,
    user: &UserAccount,
    account_id: &str,
) -> Option<StakeAccount> {
    let result = user.view(PendingContractTx::new(
        contract_account_id,
        "lookup_account",
        json!({ "account_id": account_id }),
        true,
    ));

    result.unwrap_json()
}
