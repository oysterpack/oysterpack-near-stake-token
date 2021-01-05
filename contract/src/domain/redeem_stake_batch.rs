use crate::domain::{
    BatchId, RedeemStakeBatchReceipt, StakeTokenValue, TimestampedStakeBalance, YoctoStake,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug)]
pub struct RedeemStakeBatch {
    batch_id: BatchId,
    balance: TimestampedStakeBalance,
}

impl RedeemStakeBatch {
    /// ## Panics
    /// if NEAR runtime context is not available
    pub fn new(batch_id: BatchId, balance: YoctoStake) -> Self {
        Self {
            batch_id,
            balance: TimestampedStakeBalance::new(balance),
        }
    }

    pub fn id(&self) -> BatchId {
        self.batch_id
    }

    pub fn balance(&self) -> TimestampedStakeBalance {
        self.balance
    }

    pub fn add(&mut self, amount: YoctoStake) {
        self.balance.credit(amount)
    }

    /// returns updated balance
    pub fn remove(&mut self, amount: YoctoStake) -> YoctoStake {
        self.balance.debit(amount)
    }

    pub fn create_receipt(&self, stake_token_value: StakeTokenValue) -> RedeemStakeBatchReceipt {
        RedeemStakeBatchReceipt::new(self.balance.amount(), stake_token_value)
    }
}
