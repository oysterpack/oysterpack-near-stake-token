#![allow(dead_code)]

use near_sdk::{serde_json::json, AccountId, PendingContractTx};
use near_sdk_sim::*;
use oysterpack_near_stake_token::interface::{BatchId, StakeBatchReceipt};
use oysterpack_near_stake_token::near::NO_DEPOSIT;
use oysterpack_near_stake_token::{
    domain::{YoctoNear, TGAS},
    interface,
};

pub struct StakingServiceClient {
    pub contract_account_id: AccountId,
}

impl StakingServiceClient {
    pub fn new(contract_account_id: &str) -> Self {
        Self {
            contract_account_id: contract_account_id.to_string(),
        }
    }

    pub fn staking_pool_id(&self, user: &UserAccount) -> AccountId {
        let result = user.view(PendingContractTx::new(
            &self.contract_account_id,
            "staking_pool_id",
            json!({}),
            true,
        ));
        result.unwrap_json()
    }

    pub fn stake_batch_receipt(
        &self,
        user: &UserAccount,
        batch_id: BatchId,
    ) -> Option<StakeBatchReceipt> {
        let result = user.view(PendingContractTx::new(
            &self.contract_account_id,
            "stake_batch_receipt",
            json!({ "batch_id": batch_id }),
            true,
        ));
        result.unwrap_json()
    }

    pub fn deposit(&self, user: &UserAccount, amount: interface::YoctoNear) -> interface::BatchId {
        let result = user.call(
            PendingContractTx::new(&self.contract_account_id, "deposit", json!({}), false),
            amount.value(),
            TGAS.value() * 10,
        );
        println!("deposit: {:#?}", result);
        result.unwrap_json()
    }

    pub fn stake(&self, user: &UserAccount) -> ExecutionResult {
        let result = user.call(
            PendingContractTx::new(&self.contract_account_id, "stake", json!({}), false),
            NO_DEPOSIT.value(),
            TGAS.value() * 200,
        );
        println!("stake: {:#?}", result);
        result
    }

    pub fn claim_receipts(&self, user: &UserAccount) -> ExecutionResult {
        let result = user.call(
            PendingContractTx::new(
                &self.contract_account_id,
                "claim_receipts",
                json!({}),
                false,
            ),
            NO_DEPOSIT.value(),
            TGAS.value() * 10,
        );
        println!("claim_receipts: {:#?}", result);
        result
    }

    pub fn redeem_all(&self, user: &UserAccount) -> Option<BatchId> {
        let result = user.call(
            PendingContractTx::new(&self.contract_account_id, "redeem_all", json!({}), false),
            NO_DEPOSIT.value(),
            TGAS.value() * 10,
        );
        println!("redeem_all: {:#?}", result);
        result.unwrap_json()
    }

    pub fn unstake(&self, user: &UserAccount) -> ExecutionResult {
        let result = user.call(
            PendingContractTx::new(&self.contract_account_id, "unstake", json!({}), false),
            NO_DEPOSIT.value(),
            TGAS.value() * 150,
        );
        println!("unstake: {:#?}", result);
        result
    }
}
