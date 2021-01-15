use near_sdk::{
    serde::{Deserialize, Serialize},
    serde_json::json,
    AccountId, PendingContractTx,
};
use near_sdk_sim::*;
use oysterpack_near_stake_token::interface::{contract_state::ContractState, Config};

pub struct StakingPoolClient {
    staking_pool_id: AccountId,
    stake_token_contract_id: AccountId,
}

impl StakingPoolClient {
    pub fn new(staking_pool_id: AccountId, stake_token_contract_id: AccountId) -> Self {
        Self {
            staking_pool_id,
            stake_token_contract_id,
        }
    }

    pub fn get_account(&self, user: &UserAccount) -> StakingPoolAccount {
        let result = user.view(PendingContractTx::new(
            &self.staking_pool_id,
            "get_account",
            json!({ "account_id": self.stake_token_contract_id }),
            true,
        ));

        result.unwrap_json()
    }
}

type Balance = near_sdk::json_types::U128;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolAccount {
    pub account_id: AccountId,
    /// The unstaked balance that can be withdrawn or staked.
    pub unstaked_balance: Balance,
    /// The amount balance staked at the current "stake" share price.
    pub staked_balance: Balance,
    /// Whether the unstaked balance is available for withdrawal now.
    pub can_withdraw: bool,
}
