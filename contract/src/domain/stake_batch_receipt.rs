//! Users can deposit and stake NEAR. In return, they receive STAKE tokens.
//!
//! In order to issue STAKE tokens to the account, the STAKE token value needs to be computed
//! after the deposit and stake request has been confirmed with the staking pool.
//!
//! Multiple deposit and stake requests are batched together and submitted to the staking pool
//! on a scheduled basis. The contract is locked while STAKE tokens are being issued because the
//! STAKE token value needs to be computed.

use crate::domain::{BatchId, StakeTokenValue, TimestampedNearBalance, YoctoNear};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct StakeBatchReceipt {
    batch_id: BatchId,
    staked_near: YoctoNear,
    stake_token_value: StakeTokenValue,
}
