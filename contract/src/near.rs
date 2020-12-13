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

/// checks if the first PromiseResult was successful
///
/// ## Panics
/// if there are no promise results - this should only be called if promise results are expected
#[cfg(test)]
pub fn promise_result_succeeded() -> bool {
    unsafe {
        match ENV.promise_result(0) {
            PromiseResult::Successful(_) => true,
            _ => false,
        }
    }
}

/// checks if the first PromiseResult was successful
///
/// ## Panics
/// if there are no promise results - this should only be called if promise results are expected
#[cfg(not(test))]
pub fn promise_result_succeeded() -> bool {
    match env::promise_result(0) {
        PromiseResult::Successful(_) => true,
        _ => false,
    }
}

/// # Panics
/// if there are no promise results - this should only be called if promise results are expected
#[cfg(test)]
pub fn all_promise_results_succeeded() -> bool {
    unsafe {
        let count = ENV.promise_results_count();
        assert!(count > 0, "there are no promise results");
        for i in 0..count {
            let success = match ENV.promise_result(0) {
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

/// # Panics
/// if there are no promise results - this should only be called if promise results are expected
#[cfg(not(test))]
pub fn all_promise_results_succeeded() -> bool {
    let count = env::promise_results_count();
    assert!(count > 0, "there are no promise results");
    for i in 0..count {
        let success = match env::promise_result(0) {
            PromiseResult::Successful(_) => true,
            _ => false,
        };
        if !success {
            return false;
        }
    }
    true
}

#[cfg(test)]
pub use test_env::*;

#[cfg(test)]
pub mod test_env {
    use near_sdk::{env, PromiseResult};

    pub static mut ENV: Env = Env {
        promise_results_count: env::promise_results_count,
        promise_result: env::promise_result,
    };

    /// intended to plugin a mock for unit testing
    pub fn set_env(env: Env) {
        unsafe { ENV = env }
    }

    /// abstracts away the NEAR env
    /// - this enables the Near env to be decoupled to make it easier to test
    pub struct Env {
        promise_results_count: fn() -> u64,

        promise_result: fn(u64) -> PromiseResult,
    }

    impl Env {
        pub fn new(
            promise_results_count: fn() -> u64,
            promise_result: fn(u64) -> PromiseResult,
        ) -> Self {
            Self {
                promise_results_count,
                promise_result,
            }
        }

        pub fn promise_results_count(&self) -> u64 {
            (self.promise_results_count)()
        }

        pub fn promise_result(&self, result_index: u64) -> PromiseResult {
            (self.promise_result)(result_index)
        }
    }
}
