pub mod account_management;
pub mod staking_service;
pub mod staking_service_callbacks;

pub use staking_service::*;

use crate::StakeTokenContract;
use near_sdk::env;

impl StakeTokenContract {
    /// asserts that the predecessor account ID must be the operator
    fn assert_is_operator(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.operator_id,
            "function can only be invoked by the operator"
        );
    }
}
