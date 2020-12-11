use crate::interface::{
    BatchId, BlockTimestamp, RedeemStakeBatchReceipt, StakeBatchReceipt, YoctoNear, YoctoStake,
};
use near_sdk::{AccountId, Promise, PromiseOrValue};

pub trait StakingService {
    ////////////////////////////
    ///     VIEW METHODS    ///
    /// //////////////////////

    /// returns the staking pool account ID used for the STAKE token
    fn staking_pool_id(&self) -> AccountId;

    //////////////////////////////
    ///     CHANGE METHODS    ///
    /// ////////////////////////

    /// Stakes the attached deposit.
    ///
    /// If the contract is locked, then the NEAR funds will not be staked immediately,
    /// They will be scheduled on the next available [StakeBatch].
    ///
    /// If the contract is not locked, then funds are staked with staking pool and Promise is returned.
    /// If the contract is locked, then the funds are batched, and the [BatchId] is returned.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if no deposit is attached
    ///
    /// #[payable]
    fn deposit_and_stake(&mut self) -> PromiseOrValue<BatchId>;

    /// returns false if there was no batch to run
    fn run_stake_batch(&mut self) -> PromiseOrValue<bool>;

    /// Redeem the specified amount of STAKE.
    ///
    /// If the contract is locked or if there is a pending withdrawal, then the request is batched
    /// and the [BatchId] is returned.
    ///
    /// If the contract is not locked and there is no pending withdrawal, then the redeem batch is
    /// run.
    ///
    /// This will schedule NEAR tokens to be unstaked in the
    /// next [RedeemStakeBatch]. The next batch will run when all available funds are available to
    /// be withdrawn from the staking pool.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if there is not enough STAKE in the account to fulfill the request
    fn redeem(&mut self, amount: YoctoStake) -> PromiseOrValue<BatchId>;

    /// Redeems all available STAKE - see [redeem]
    ///
    /// Returns the total amount of STAKE that was redeemed.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if the account has no STAKE to redeem
    fn redeem_all(&mut self) -> PromiseOrValue<BatchId>;

    /// Returns false if there was no pending request.
    fn cancel_pending_redeem_stake_request(&mut self) -> bool;

    /// returns false if there was no batch to run
    fn run_redeem_stake_batch(&mut self) -> PromiseOrValue<bool>;

    /// Explicitly claims any available funds for batch receipts:
    /// - updates STAKE and NEAR account balances
    ///
    /// NOTE: batch receipts claims are checks are included for every stake and redeem request
    fn claim_all_batch_receipt_funds(&mut self);

    /// Returns the batch that is awaiting for funds to be available to be withdrawn.
    ///
    /// NOTE: pending withdrawals blocks [RedeemStakeBatch] to run
    fn pending_redeem_stake_batch_receipt(&self) -> Option<RedeemStakeBatchReceipt>;
}
