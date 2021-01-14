use near_sdk::{json_types::U128, AccountId};
use near_sdk::{serde_json::json, PendingContractTx};
use near_sdk_sim::*;
use oysterpack_near_stake_token::domain::{Gas, YoctoNear};
use oysterpack_near_stake_token::interface::StakeAccount;
use oysterpack_near_stake_token::near::NO_DEPOSIT;

pub fn owner_id(contract_account_id: &str, user: &UserAccount) -> AccountId {
    let result = user.view(PendingContractTx::new(
        contract_account_id,
        "owner_id",
        json!({}),
        true,
    ));

    result.unwrap_json()
}
