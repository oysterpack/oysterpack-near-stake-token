use crate::interface::{BatchId, RedeemStakeBatchReceipt, StakeTokenValue, YoctoNear, YoctoStake};
use near_sdk::{AccountId, Promise};

pub trait StakingService {
    ////////////////////////////
    ///     VIEW METHODS    ///
    /// //////////////////////

    /// returns the staking pool account ID used for the STAKE token
    fn staking_pool_id(&self) -> AccountId;

    /// Returns the cached STAKE token value which is computed from the total STAKE token supply
    /// and the staked NEAR account balance with the staking pool:
    ///
    /// STAKE Token Value = (Total Staked NEAR balance) / (Total STAKE token supply)
    ///
    /// Stake rewards are applied once per epoch time period. Thus, the STAKE token value remains
    /// constant until stake rewards are issued. Based on how stake rewards work, it is safe to
    /// cache the [StakeTokenValue] until the epoch changes.
    ///
    /// Thus, the STAKE token value only changes when the epoch rolls.
    ///
    /// NOTE: the STAKE token value is refreshed each time a batch is run. It can also be manually
    /// refreshed via [refresh_stake_token_value()]
    fn stake_token_value(&self) -> StakeTokenValue;

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

    /// locks the contract to stake the batched NEAR funds and then kicks off the workflow
    /// 1. gets the account stake balance from the staking pool
    /// 2. updates STAKE token value
    /// 3. deposits and stakes the NEAR funds with the staking pool
    /// 4. creates the batch receipt
    /// 5. releases the lock
    ///
    /// NOTE: takes 5 blocks to complete
    ///
    /// ## Panics
    /// - if contract is locked for
    ///   - staking batch is in progress
    ///   - unstaking is in progress
    /// - if there is no stake batch to run
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

    /// STAKE tokens are redeemed in 2 steps: first the corresponding NEAR is unstaked with the staking
    /// pool. Second, the NEAR funds need to be withdrawn from the staking pool. The unstaked NEAR
    /// funds are not immediately available. They are locked in the staking pool for 4 epochs. Further
    /// STAKE funds cannot be redeemed until the unstaked NEAR funds are withdrawn from the staking
    /// pool. Otherwise, the lock period's clock resets to another 4 epochs.
    ///
    /// ## unstaking workflow
    /// 1. sets the redeem lock to Unstaking
    /// 2. get account info from staking pool
    /// 3. if unstaked balance > 0
    ///    3.1 if unstaked NEAR can be withdrawn, the withdraw all funds from the staking pool
    ///    3.2 then go back to step #2
    /// 4. if unstaked balance == 0, then unstake the NEAR with the saking pool
    /// 5. create batch receipt
    /// 6. set redeem lock to `PendingWithdrawal`
    /// 7. clear redeem lock if lock state is `Unstaking` - which means a workflow step failed
    ///
    /// ## pending withdrawal workflow
    /// 1. get account info from staking pool
    /// 2. if unstaked balance is > 0 and unstaked NEAR can be withdrawm,
    ///    2.1 then withdraw all
    ///    2.2 then go back to step #1
    /// 3. If unstaked balance == 0, then
    ///    3.1 set redeem lock to None
    ///    3.2 pop redeem stake batch
    ///
    /// ## Panics
    /// - if staking is in progress
    /// - if the redeem stake batch is already in progress
    /// - if unstaked funds are not available for withdrawal
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

    /// refreshes the staked balance and updates the cached STAKE token value
    ///
    /// Promise returns: [StakeTokenValue]
    fn refresh_stake_token_value(&self) -> Promise;
}
