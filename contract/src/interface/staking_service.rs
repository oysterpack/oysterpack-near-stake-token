use crate::interface::{
    BatchId, BlockTimestamp, RedeemStakeBatchReceipt, StakeBatchReceipt, StakeTokenValue,
    YoctoNear, YoctoStake,
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

    /// Adds the attached deposit to the next [StakeBatch] scheduled to run.
    ///
    /// Returns the [BatchId] for the [StakeBatch] that the funds are deposited into.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if no deposit is attached
    ///
    /// #[payable]
    fn deposit(&mut self) -> BatchId;

    /// withdraws specified amount from stake batch funds and refunds the account
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are insufficient funds to fulfill the request
    /// - if the contract is locked
    fn withdraw_funds_from_stake_batch(&mut self, amount: YoctoNear);

    /// withdraws all NEAR from stake batch funds and refunds the account
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are funds batched
    /// - if the contract is locked
    fn withdraw_all_funds_from_stake_batch(&mut self);

    ///
    /// ## Panics
    /// - if contract is locked, which means a batch run is in progress
    /// - if there is no stake batch to run
    ///
    /// ## Notes
    /// - takes 5 blocks to complete:
    ///     1. StakeTokenContract::run_stake_batch
    ///     2. StakingPool::get_account_staked_balance
    ///     3. StakeTokenContract::on_get_account_staked_balance_to_run_stake_batch
    ///     4. StakingPool:deposit_and_stake
    ///     5. StakeTokenContract::on_deposit_and_stake
    fn run_stake_batch(&mut self) -> Promise;

    /// Redeem the specified amount of STAKE.
    ///
    /// If the contract is locked for redeeming, then the request is put into the next batch.    ///
    /// If the contract is not locked for redeeming, then the request is put into the current batch.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if there is not enough STAKE in the account to fulfill the request
    fn redeem(&mut self, amount: YoctoStake) -> BatchId;

    /// Redeems all available STAKE - see [redeem]
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if the account has no STAKE to redeem
    fn redeem_all(&mut self) -> BatchId;

    /// Returns false if there was no pending request.
    fn cancel_pending_redeem_stake_request(&mut self) -> bool;

    /// returns None if there was no batch to run
    fn run_redeem_stake_batch(&mut self) -> Promise;

    /// Explicitly claims any available funds for batch receipts:
    /// - updates STAKE and NEAR account balances
    ///
    /// NOTE: batch receipts claims are checks are included for every stake and redeem request
    fn claim_all_batch_receipt_funds(&mut self);

    /// Returns the batch that is awaiting for funds to be available to be withdrawn.
    ///
    /// NOTE: pending withdrawals blocks [RedeemStakeBatch] to run
    fn pending_redeem_stake_batch_receipt(&self) -> Option<RedeemStakeBatchReceipt>;

    /// Returns the current STAKE token value which is computed from the total STAKE token supply
    /// and the staked NEAR account balance with the staking pool:
    ///
    /// STAKE Token Value = (Total Staked NEAR balance) / (Total STAKE token supply)
    ///
    /// Stake rewards are applied once per epoch time period. Thus, the STAKE token value remains
    /// constant until stake rewards are issued. Based on how stake rewards work, it is safe to
    /// cache the [StakeTokenValue] until the epoch changes.
    ///
    /// In summary, the STAKE token value can be cached for the epoch time period.
    fn stake_token_value(&self) -> PromiseOrValue<StakeTokenValue>;

    /// always refreshes the staked balance and updates the cached STAKE token value
    ///
    /// Promise returns: [StakeTokenValue]
    fn refresh_stake_token_value(&self) -> Promise;
}
