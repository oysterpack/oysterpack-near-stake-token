pub mod account_management;
pub mod contract_owner;
pub mod operator;
pub mod redeeming_workflow_callbacks;
pub mod settings;
pub mod staking_service;
pub mod staking_workflow_callbacks;
pub mod vault_fungible_token;

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

#[cfg(not(test))]
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

/// in order to make it easier to unit test Promise func callbacks, we need to abstract away the near env
#[cfg(test)]
impl StakeTokenContract {
    /// checks if the first PromiseResult was successful
    ///
    /// ## Panics
    /// if there are no promise results - this should only be called if promise results are expected
    pub fn promise_result_succeeded(&self) -> bool {
        match self.env.promise_result(0) {
            PromiseResult::Successful(_) => true,
            _ => false,
        }
    }

    /// # Panics
    /// if there are no promise results - this should only be called if promise results are expected
    pub fn all_promise_results_succeeded(&self) -> bool {
        let count = self.env.promise_results_count();
        assert!(count > 0, "there are no promise results");
        for _i in 0..count {
            let success = match self.env.promise_result(0) {
                PromiseResult::Successful(_) => true,
                _ => false,
            };
            if !success {
                return false;
            }
        }
        true
    }

    pub fn set_env(&mut self, env: near_env::Env) {
        self.env = env;
    }
}

#[cfg(test)]
pub(crate) mod near_env {
    use near_sdk::PromiseResult;

    /// abstracts away the NEAR env
    /// - this enables the Near env to be decoupled to make it easier to test
    pub struct Env {
        pub promise_results_count_: fn() -> u64,
        pub promise_result_: fn(u64) -> PromiseResult,
    }

    impl Env {
        pub fn promise_results_count(&self) -> u64 {
            (self.promise_results_count_)()
        }

        pub fn promise_result(&self, result_index: u64) -> PromiseResult {
            (self.promise_result_)(result_index)
        }
    }

    impl Default for Env {
        fn default() -> Self {
            Self {
                promise_results_count_: near_sdk::env::promise_results_count,
                promise_result_: near_sdk::env::promise_result,
            }
        }
    }
}
