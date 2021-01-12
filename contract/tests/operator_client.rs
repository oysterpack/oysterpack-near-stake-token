use near_sdk::{serde_json::json, PendingContractTx};
use near_sdk_sim::*;
use oysterpack_near_stake_token::interface::contract_state::ContractState;
use oysterpack_near_stake_token::interface::Config;

pub fn contract_state(contract_account_id: &str, user: &UserAccount) -> ContractState {
    let result = user.view(PendingContractTx::new(
        contract_account_id,
        "contract_state",
        json!({}),
        true,
    ));

    result.unwrap_json()
}

pub fn config(contract_account_id: &str, user: &UserAccount) -> Config {
    let result = user.view(PendingContractTx::new(
        contract_account_id,
        "config",
        json!({}),
        true,
    ));

    result.unwrap_json()
}
