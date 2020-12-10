//! Users can deposit and stake NEAR. In return, they receive STAKE tokens.
//!
//! In order to issue STAKE tokens to the account, the STAKE token value needs to be computed
//! after the deposit and stake request has been confirmed with the staking pool.
//!
//! Multiple deposit and stake requests are batched together and submitted to the staking pool
//! on a scheduled basis. The contract is locked while STAKE tokens are being issued because the
//! STAKE token value needs to be computed.

use crate::domain::{BatchId, TimestampedNearBalance, YoctoNear};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

/// Gathers NEAR deposits to stake into a batch
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Default)]
pub struct StakeBatch {
    batch_id: BatchId,
    balance: TimestampedNearBalance,
}

impl StakeBatch {
    pub fn new(batch_id: BatchId, amount: YoctoNear) -> Self {
        Self {
            batch_id,
            balance: TimestampedNearBalance::new(amount),
        }
    }

    pub fn id(&self) -> BatchId {
        self.batch_id
    }

    pub fn balance(&self) -> TimestampedNearBalance {
        self.balance
    }

    pub fn add(&mut self, amount: YoctoNear) {
        self.balance.credit(amount)
    }

    pub fn remove(&mut self, amount: YoctoNear) {
        self.balance.debit(amount)
    }
}
