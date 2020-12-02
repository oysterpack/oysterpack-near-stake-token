use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    json_types::U128,
    AccountId, Balance, PromiseResult,
};
use std::ops::Deref;

pub const YOCTO: u128 = 1_000_000_000_000_000_000_000_000;

pub const ZERO_BALANCE: Balance = 0;

pub const NO_DEPOSIT: Balance = 0;

pub mod json_types {
    use near_sdk::json_types::{U128, U64};

    pub type YoctoNEAR = U128;
    pub type YoctoSTAKE = U128;

    pub type BlockHeight = U64;
    pub type BlockTimestamp = U64;

    pub type Balance = U128;
}

pub type StakingPoolId = AccountId;
pub type BlockTimestamp = u64;

/// asserts that predecessor account is the contract itself - used to enforce that callbacks
/// should only be called internally - even though they are exposed on the public contract interface
pub fn assert_predecessor_is_self() {
    assert_eq!(env::predecessor_account_id(), env::current_account_id());
}

pub fn is_promise_result_success(result: PromiseResult) -> bool {
    match result {
        PromiseResult::Successful(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::test_utils::near;
    use near_sdk::{testing_env, MockedBlockchain, VMContext};
}
