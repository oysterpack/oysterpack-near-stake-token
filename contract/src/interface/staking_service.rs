use crate::interface::{
    BatchId, RedeemStakeBatchReceipt, StakeBatchReceipt, YoctoNear, YoctoStake,
};
use near_sdk::{AccountId, Promise};

/// Integrates with the staking pool contract and manages the staking/unstaking workflows.
pub trait StakingService {
    /// returns the staking pool account ID used for the STAKE token
    /// - this is the staking pool that this contract is linked to
    fn staking_pool_id(&self) -> AccountId;

    /// looks up the receipt for the specified batch ID
    /// - when a batch is successfully processed a receipt is created, meaning the NEAR funds have
    ///   been successfully deposited and staked with the staking pool
    /// - the receipt is used by customer accounts to claim STAKE tokens for their staked NEAR based
    ///   on the STAKE token value at the point in time when the batch was run
    fn stake_batch_receipt(&self, batch_id: BatchId) -> Option<StakeBatchReceipt>;

    /// looks up the receipt for the specified batch ID
    /// - when a batch is successfully processed a receipt is created, meaning the unstaked NEAR
    ///   has been withdrawn from the staking pool contract
    /// - the receipt is used by customer accounts to claim the unstaked NEAR tokens for their
    ///   redeemed STAKE tokens based on the STAKE token value at the point in time when the batch
    ///   was run
    fn redeem_stake_batch_receipt(&self, batch_id: BatchId) -> Option<RedeemStakeBatchReceipt>;

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
    /// NOTE: all batch receipts are first claimed
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are insufficient funds to fulfill the request
    /// - if the contract is locked
    fn withdraw_funds_from_stake_batch(&mut self, amount: YoctoNear);

    /// withdraws all NEAR from stake batch funds and refunds the account
    ///
    /// NOTE: all batch receipts are first claimed
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if the contract is locked
    fn withdraw_all_funds_from_stake_batch(&mut self);

    /// locks the contract to stake the batched NEAR funds and then kicks off the workflow
    /// 1. lock the contract
    /// 2. gets the account stake balance from the staking pool
    /// 3. updates STAKE token value
    /// 4. deposits and stakes the NEAR funds with the staking pool
    /// 5. creates the batch receipt
    /// 6. releases the lock
    ///
    /// ## Panics
    /// - if contract is locked for
    ///   - staking batch is in progress
    ///   - unstaking is in progress
    /// - if there is no stake batch to run
    fn run_stake_batch(&mut self) -> Promise;

    /// Redeem the specified amount of STAKE. The STAKE is not immediately redeemed. The redeem
    /// request is placed into a batch. The account's STAKE balance is debited the amount and moved
    /// into the batch.
    /// - [run_redeem_stake_batch] is used to run the batch and redeem the funds from the staking pool
    ///
    /// If the contract is locked for redeeming, then the request is put into the next batch.    
    /// If the contract is not locked for redeeming, then the request is put into the current batch,
    /// i.e. the amount is added to the current batch.
    ///
    /// Returns the batch ID that the request is batched into.
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
    /// - STAKE funds that were locked in the redeem stake batch are made available for transfer
    fn cancel_pending_redeem_stake_request(&mut self) -> bool;

    /// STAKE tokens are redeemed in 2 steps: first the corresponding NEAR is unstaked with the staking
    /// pool. Second, the NEAR funds need to be withdrawn from the staking pool. The unstaked NEAR
    /// funds are not immediately available. They are locked in the staking pool for 4 epochs. Further
    /// STAKE funds cannot be redeemed, i.e., unstaked from the staking pool, until the unstaked NEAR
    /// funds are withdrawn from the staking pool. Otherwise, the lock period's clock resets to
    /// another 4 epochs.
    ///
    /// ## unstaking workflow
    /// 1. locks the contract for unstaking
    /// 2. get account staked balance from staking pool
    /// 3. update the STAKE token value and compute amount of NEAR funds to unstake
    /// 4. submit unstake request to staking pool
    /// 5. create batch receipt
    /// 6. set redeem lock to `PendingWithdrawal`
    /// 7. clear redeem lock if lock state is `Unstaking` - which means a workflow step failed
    ///
    /// ## pending withdrawal workflow
    /// 1. get account info from staking pool
    /// 2. if unstaked balance is > 0 and unstaked NEAR can be withdrawn:
    ///    2.1 then withdraw all
    /// 3. finalize the redeem stake batch
    ///    3.1 update the total NEAR available balance
    ///    3.2 set redeem lock to None
    ///    3.3 pop redeem stake batch
    ///
    /// ## Panics
    /// - if staking is in progress
    /// - if the redeem stake batch is already in progress
    /// - if pending withdrawal and unstaked funds are not available for withdrawal
    fn run_redeem_stake_batch(&mut self) -> Promise;

    /// Returns the batch that is awaiting for funds to be available to be withdrawn.
    ///
    /// NOTE: pending withdrawals blocks [RedeemStakeBatch] to run
    fn pending_withdrawal(&self) -> Option<RedeemStakeBatchReceipt>;
}
