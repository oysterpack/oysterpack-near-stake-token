#![allow(dead_code)]

use near_sdk::{serde_json::json, AccountId, PendingContractTx};
use near_sdk_sim::*;
use oysterpack_near_stake_token::{
    domain::{Gas, YoctoNear},
    interface::{Config, ContractBalances},
};

pub struct FinancialsClient {
    contract_account_id: AccountId,
}

impl FinancialsClient {
    pub fn new(contract_account_id: &str) -> Self {
        Self {
            contract_account_id: contract_account_id.to_string(),
        }
    }

    pub fn balances(&self, user: &UserAccount) -> ContractBalances {
        let result = user.view(PendingContractTx::new(
            &self.contract_account_id,
            "balances",
            json!({}),
            true,
        ));

        result.unwrap_json()
    }

    pub fn deposit_earnings(
        &self,
        user: &UserAccount,
        deposit: YoctoNear,
        gas: Gas,
    ) -> ContractBalances {
        let result = user.call(
            PendingContractTx::new(
                &self.contract_account_id,
                "deposit_earnings",
                json!({}),
                false,
            ),
            deposit.value(),
            gas.value(),
        );

        result.unwrap_json()
    }
}
