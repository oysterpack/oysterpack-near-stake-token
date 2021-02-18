//! NEAR specific constants and logging support

pub mod storage_keys;

use crate::domain::{EpochHeight, YoctoNear};
use near_sdk::env;
use std::fmt::Debug;

/// YOCTO = 10^24
pub const YOCTO: u128 = 1_000_000_000_000_000_000_000_000;

/// Used to indicate that no deposit is being attached to a cross contract func call
pub const NO_DEPOSIT: YoctoNear = YoctoNear(0);

/// how many epochs unstaked NEAR funds are held before they are available for withdrawal as defined
/// per the NEAR protocol
/// - https://docs.near.org/docs/validator/delegation#b-withdraw-the-tokens
/// - https://github.com/near/core-contracts/blob/master/staking-pool/src/internal.rs
///  - `account.unstaked_available_epoch_height = env::epoch_height() + NUM_EPOCHS_TO_UNLOCK;`
///
/// - https://github.com/near/core-contracts/blob/master/staking-pool/src/lib.rs
///  - `const NUM_EPOCHS_TO_UNLOCK: EpochHeight = 4;`
pub const UNSTAKED_NEAR_FUNDS_NUM_EPOCHS_TO_UNLOCK: EpochHeight = EpochHeight(4);

/// wrapper around `near_sdk::env::log()` which supports structured logging
pub fn log<T: Debug>(event: T) {
    env::log(format!("{:#?}", event).as_bytes());
}

/// used to protect functions that transfer value against FCAK calls
pub(crate) fn assert_yocto_near_attached() {
    assert_eq!(
        env::attached_deposit(),
        1,
        "exactly 1 yoctoNEAR must be attached"
    )
}
