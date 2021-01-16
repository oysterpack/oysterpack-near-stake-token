use near_sdk::{serde_json::json, AccountId, PendingContractTx};
use near_sdk_sim::*;
use oysterpack_near_stake_token::interface::contract_state::ContractState;
use oysterpack_near_stake_token::interface::Config;

pub struct OperatorClient {
    contract_account_id: AccountId,
}

impl OperatorClient {
    pub fn new(contract_account_id: &str) -> Self {
        Self {
            contract_account_id: contract_account_id.to_string(),
        }
    }

    pub fn contract_state(&self, user: &UserAccount) -> ContractState {
        let result = user.view(PendingContractTx::new(
            &self.contract_account_id,
            "contract_state",
            json!({}),
            true,
        ));

        let state: ContractState = result.unwrap_json();
        println!(
            "{}",
            near_sdk::serde_json::to_string_pretty(&state).unwrap()
        );
        state
    }

    pub fn config(&self, user: &UserAccount) -> Config {
        let result = user.view(PendingContractTx::new(
            &self.contract_account_id,
            "config",
            json!({}),
            true,
        ));

        result.unwrap_json()
    }
}
