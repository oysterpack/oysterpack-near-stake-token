#![allow(dead_code)]

use near_sdk::{
    serde::{Deserialize, Serialize},
    serde_json::json,
    AccountId, PendingContractTx,
};
use near_sdk_sim::*;
use oysterpack_near_stake_token::domain::TGAS;
use oysterpack_near_stake_token::interface::{contract_state::ContractState, Config};

pub struct StakingPoolClient {
    staking_pool_id: AccountId,
    stake_token_contract_id: AccountId,
}

impl StakingPoolClient {
    pub fn new(staking_pool_id: &str, stake_token_contract_id: &str) -> Self {
        Self {
            staking_pool_id: staking_pool_id.to_string(),
            stake_token_contract_id: stake_token_contract_id.to_string(),
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

    pub fn update_account(&self, user: &UserAccount, account: StakingPoolAccount) {
        let result = user.call(
            PendingContractTx::new(
                &self.staking_pool_id,
                "update_account",
                json!({ "account": account }),
                false,
            ),
            0,
            TGAS.value() * 100,
        );
        result.assert_success();
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
