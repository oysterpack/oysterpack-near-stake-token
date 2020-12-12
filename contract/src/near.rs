pub mod storage_keys;
use near_sdk::{env, PromiseResult};

pub const YOCTO: u128 = 1_000_000_000_000_000_000_000_000;

pub const NO_DEPOSIT: u128 = 0;

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
