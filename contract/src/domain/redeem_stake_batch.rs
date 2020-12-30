//! Redeeming STAKE for NEAR goes through a workflow process outlined below.
//!
//! ## How Unstaking and Withrawal Works With Staking Pool Contracts
//! When users unstake NEAR with the current version of the [staking pool contract], users must wait
//! 4 epochs before the funds are available for withdrawal. If the user submits another unstake request
//! during the 4 epoch wait period, the wait period is reset again to 4 epochs. For example, if a user
//! unstakes 100 NEAR at epoch 10, then the funds would be available for withdrawal ar epoch 14. However,
//! if a user then submits another request to unstake an additional 50 NEAR in epoch 13, the 100 NEAR
//! funds will no longer be available at epoch 14. All 150 NEAR will then become available for withdrawal
//! at epoch 17.
//!
//! The process to redeem STAKE tokens for NEAR also needs to take into the following factors:
//! - the single contract account stakes on the behalf of multiple NEAR accounts
//! - the contract needs to compute the STAKE token value in NEAR at the point in time before submitting
//!   the unstake request to the staking pool. In order to compute STAKE value, the contract will be
//!   locked, which means that accounts will not be able to do the following:
//!   - deposit and stake NEAR funds
//!   - lock STAKE to redeem
//!
//! ## Redeem STAKE Workflow
//! [RedeemStakeBatch] -> [UnstakeBatch]
//!
//! 1. Users lock STAKE tokens into an [RedeemStakeBatch] to redeem
//!  - STAKE tokens moved into the [RedeemStakeBatch] are said to be locked because they cannot be
//!    transferred to other other accounts.
//! 2. Invoke the `run_redeem_stake_batch` contract function
//!    - the [RedeemStakeBatch] will only be processed if there are no available unstaked funds to
//!      withdraw from the staking pool
//!    - if there are STAKE funds to redeem then lock the contract.
//!    - if there are (staking, redeeming) transactions in flight, then the return result will
//!      indicate that the contract is waiting for pending transactions to complete.
//!      `run_redeem_stake_batch` will need to invoked again after all pending transactions have completed.
//!    - if there are no pending transtions, then compute the STAKE value in NEAR and create
//!      [UnstakeBatch], which is used to submit the unstake request to the staking pool
//!
//! OysterPack will schedule a background process to check periodically (every 30 mins) if there
//! are STAKE tokens to redeem and kick off the process.
//! - NOTE: accounts will be able to invoke the process themselves, but then they will be responsible
//!   to pay the transaction gas fees.
//!
//! [staking pool contract] = https://github.com/near/core-contracts/tree/master/staking-pool

use crate::domain::{BatchId, TimestampedStakeBalance, YoctoStake};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug)]
pub struct RedeemStakeBatch {
    batch_id: BatchId,
    balance: TimestampedStakeBalance,
}

impl RedeemStakeBatch {
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
}
