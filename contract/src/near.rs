pub mod storage_keys;
use crate::domain::YoctoNear;
use near_sdk::{env, PromiseResult};

/// YOCTO = 10^24
pub const YOCTO: u128 = 1_000_000_000_000_000_000_000_000;

/// Used to indicate that no deposit is being attached to a cross contract func call
pub const NO_DEPOSIT: YoctoNear = YoctoNear(0);

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
