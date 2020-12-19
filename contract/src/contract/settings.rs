use crate::config::Config;
use crate::errors::asserts::OPERATOR_ID_MUST_NOT_BE_CONTRACT_ID;
use near_sdk::{
    env,
    json_types::ValidAccountId,
    serde::{Deserialize, Serialize},
    AccountId,
};
use std::convert::TryInto;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractSettings {
    pub staking_pool_id: ValidAccountId,
    pub config: Option<Config>,
    pub operator_id: ValidAccountId,
}

impl ContractSettings {
    /// depends on NEAR runtime env
    pub fn new(staking_pool_id: AccountId, operator_id: AccountId, config: Option<Config>) -> Self {
        Self {
            staking_pool_id: staking_pool_id
                .try_into()
                .expect("invalid staking pool account ID"),
            config,
            operator_id: operator_id.try_into().expect("invalid operator account ID"),
        }
    }

    /// panics if validation fails
    pub fn validate(&self) {
        if env::current_account_id().as_str() == self.operator_id.as_ref().as_str() {
            panic!(OPERATOR_ID_MUST_NOT_BE_CONTRACT_ID);
        }
    }
}
