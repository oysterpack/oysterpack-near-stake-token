pub mod account_management;
pub mod contract_owner;
pub mod fungible_token;
pub mod operator;
pub mod redeeming_workflow_callbacks;
pub mod settings;
pub mod staking_service;
pub mod staking_workflow_callbacks;

pub use staking_service::*;

use crate::errors::asserts::{
    PREDECESSOR_MUST_BE_OPERATOR, PREDECESSOR_MUST_BE_OWNER, PREDECESSOR_MUST_NE_SELF_OR_OPERATOR,
};
use crate::StakeTokenContract;
use near_sdk::{env, PromiseResult};

impl StakeTokenContract {
    pub fn assert_predecessor_is_self_or_operator(&self) {
        let predecessor_account_id = env::predecessor_account_id();
        assert!(
            predecessor_account_id == env::current_account_id()
                || predecessor_account_id == self.operator_id,
            PREDECESSOR_MUST_NE_SELF_OR_OPERATOR
        );
    }

    pub fn assert_predecessor_is_operator(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.operator_id,
            "{}",
            PREDECESSOR_MUST_BE_OPERATOR
        );
    }

    pub fn assert_predecessor_is_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "{}",
            PREDECESSOR_MUST_BE_OWNER
        );
    }
}

impl StakeTokenContract {
    /// checks if the first PromiseResult was successful
    ///
    /// ## Panics
    /// if there are no promise results - this should only be called if promise results are expected
    pub fn promise_result_succeeded(&self) -> bool {
        match env::promise_result(0) {
            PromiseResult::Successful(_) => true,
            _ => false,
        }
    }

    /// # Panics
    /// if there are no promise results - this should only be called if promise results are expected
    pub fn all_promise_results_succeeded(&self) -> bool {
        let count = env::promise_results_count();
        assert!(count > 0, "there are no promise results");
        for i in 0..count {
            let success = match env::promise_result(i) {
                PromiseResult::Successful(_) => true,
                _ => false,
            };
            if !success {
                return false;
            }
        }
        true
    }
}
