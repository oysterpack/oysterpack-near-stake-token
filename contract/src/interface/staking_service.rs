use crate::interface::{
    BlockTimestamp, RedeemStakeBatchReceipt, StakeBatchReceipt, YoctoNear, YoctoStake,
};
use near_sdk::AccountId;

/// Functionality
trait StakingService {
    ////////////////////////////
    ///     VIEW METHODS    ///
    /// //////////////////////

    /// returns the staking pool account ID used for the STAKE token
    fn staking_pool_id(&self) -> AccountId;

    //////////////////////////////
    ///     CHANGE METHODS    ///
    /// ////////////////////////

    /// If the contract is locked, then the NEAR funds will not be staked immediately,
    /// They will be scheduled on the next available [StakeBatch].
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if no deposit is attached
    fn stake(&mut self);

    /// Redeem the specified amount of STAKE. This will schedule NEAR tokens to be unstaked in the
    /// next [RedeemStakeBatch]. The next batch will run when all available funds are available to
    /// be withdrawn from the staking pool.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if there is not enough STAKE in the account to fulfill the request
    /// - if the contract is locked and there are no NEAR funds staked in the next batch
    fn redeem(&mut self, amount: YoctoStake);

    /// Redeems all available STAKE.
    /// - see [redeem]
    ///
    /// ## Panics
    /// if account is not registered
    fn redeem_all(&mut self);

    /// Explicitly claims any available funds for batch receipts:
    /// - updates STAKE and NEAR account balances
    ///
    /// NOTE: batch receipts claims are checks are included for every stake and redeem request
    fn claim_all_batch_receipt_funds(&mut self);

    fn pending_redeem_stake_batch_receipt(&self) -> Option<RedeemStakeBatchReceipt>;
}
