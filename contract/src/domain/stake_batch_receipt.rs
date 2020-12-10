//! Users can deposit and stake NEAR. In return, they receive STAKE tokens.
//!
//! In order to issue STAKE tokens to the account, the STAKE token value needs to be computed
//! after the deposit and stake request has been confirmed with the staking pool.
//!
//! Multiple deposit and stake requests are batched together and submitted to the staking pool
//! on a scheduled basis. The contract is locked while STAKE tokens are being issued because the
//! STAKE token value needs to be computed.

use crate::domain::{Account, BatchId, StakeTokenValue, TimestampedNearBalance, YoctoNear};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct StakeBatchReceipt {
    staked_near: YoctoNear,
    stake_token_value: StakeTokenValue,
}

impl StakeBatchReceipt {
    pub fn staked_near(&self) -> YoctoNear {
        self.staked_near
    }

    pub fn stake_token_value(&self) -> StakeTokenValue {
        self.stake_token_value
    }

    /// Used to track when an account has claimed their STAKE tokens for the NEAR they have staked
    pub fn stake_tokens_issued(&mut self, staked_near: YoctoNear) {
        self.staked_near -= staked_near;
    }

    pub fn all_claimed(&self) -> bool {
        self.staked_near.value() > 0
    }
}
