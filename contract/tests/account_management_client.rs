#![allow(dead_code)]

use near_sdk::json_types::U128;
use near_sdk::{serde_json::json, AccountId, PendingContractTx};
use near_sdk_sim::*;
use oysterpack_near_stake_token::domain::{Gas, YoctoNear};
use oysterpack_near_stake_token::interface::StakeAccount;
use oysterpack_near_stake_token::near::NO_DEPOSIT;

pub struct AccountManagementClient {
    contract_account_id: AccountId,
}

impl AccountManagementClient {
    pub fn new(contract_account_id: &str) -> Self {
        Self {
            contract_account_id: contract_account_id.to_string(),
        }
    }

    pub fn register_account(
        &self,
        user: &UserAccount,
        deposit: YoctoNear,
        gas: Gas,
    ) -> ExecutionResult {
        let result = user.call(
            PendingContractTx::new(
                &self.contract_account_id,
                "register_account",
                json!({}),
                false,
            ),
            deposit.value(),
            gas.value(),
        );
        println!("register_account: {:#?}", result);
        result
    }

    pub fn unregister_account(&self, user: &UserAccount, gas: Gas) -> ExecutionResult {
        let result = user.call(
            PendingContractTx::new(
                &self.contract_account_id,
                "unregister_account",
                json!({}),
                false,
            ),
            NO_DEPOSIT.value(),
            gas.value(),
        );
        println!("unregister_account: {:#?}", result);
        result
    }

    pub fn account_storage_fee(&self, user: &UserAccount) -> YoctoNear {
        let result = user.view(PendingContractTx::new(
            &self.contract_account_id,
            "account_storage_fee",
            json!({}),
            true,
        ));

        let count: U128 = result.unwrap_json();
        count.0.into()
    }

    pub fn account_registered(&self, user: &UserAccount, account_id: &str) -> bool {
        let result = user.view(PendingContractTx::new(
            &self.contract_account_id,
            "account_registered",
            json!({ "account_id": account_id }),
            true,
        ));

        result.unwrap_json()
    }

    pub fn total_registered_accounts(&self, user: &UserAccount) -> u128 {
        let result = user.view(PendingContractTx::new(
            &self.contract_account_id,
            "total_registered_accounts",
            json!({}),
            true,
        ));

        let count: U128 = result.unwrap_json();
        count.into()
    }

    pub fn lookup_account(&self, user: &UserAccount, account_id: &str) -> Option<StakeAccount> {
        let result = user.view(PendingContractTx::new(
            &self.contract_account_id,
            "lookup_account",
            json!({ "account_id": account_id }),
            true,
        ));

        result.unwrap_json()
    }
}
