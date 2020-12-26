use crate::interface::{
    BatchId, RedeemStakeBatchReceipt, StakeBatchReceipt, YoctoNear, YoctoStake,
};
use near_sdk::{AccountId, Promise};

/// Integrates with the staking pool contract and manages STAKE token assets. The main use
/// cases supported by this interface are:
/// 1. Users can [deposit](StakingService::deposit) NEAR funds to stake.
/// 2. Users can withdraw NEAR funds from [StakeBatch](crate::interface::StakeBatch) that has not yet run.
/// 3. Once the NEAR funds are staked, the account is issued STAKE tokens based on the STAKE token
///    value computed when the [StakeBatch](crate::interface::StakeBatch) is run.
/// 4. Users can [redeem](StakingService::redeeem) STAKE tokens for NEAR.
/// 5. Users can cancel their requests to redeem STAKE, i.e., the [RedeemStakeBatch](crate::interface::RedeemStakeBatch)
///    is cancelled.
/// 6. [StakeAccount](crate::interface::StakeAccount) info can be looked up
/// 7. [RedeemStakeBatchReceipt](crate::interface::RedeemStakeBatchReceipt) information for pending staking pool
///    withdrawals for unstaked NEAR can be looked up
/// 8. Batch receipts can be looked for any active receipts which contain unclaimed funds.
///
/// ## How Staking NEAR Works
/// Users [deposit](StakingService::deposit) NEAR funds into a [StakeBatch](crate::interface::StakeBatch).
/// The batch workflow is run via [run_stake_batch()](StakingService::run_stake_batch).
/// The batch will be scheduled to run on a periodic basis - it should be run at least once per epoch
/// (every 12 hours). An off-chain process will be required to schedule the batch to run -  but anyone
/// can manually invoke [run_stake_batch()](StakingService::run_stake_batch) on the contract. When the
/// batch is run, the STAKE token value at that point in time is computed and recorded into a
/// [StakeBatchReceipt](crate::interface::StakeBatchReceipt). The STAKE tokens are issued on demand when
/// the user accounts access the contract for actions that involve STAKE tokens - when staking NEAR,
/// redeeming STAKE, or withdrawing unstaked NEAR.
///
/// ## How Redeeming STAKE Works
/// Users submit requests to [redeem](StakingService::redeem) STAKE tokens, which are collected into
/// a [RedeemStakeBatch](crate::interface::RedeemStakeBatch). The batch workflow is run via
/// [run_redeem_stake_batch()](StakingService::run_redeem_stake_batch). The batch will be scheduled to
/// run on a periodic basis - it should be scheduled to run at least once per epoch (every 12 hours).
/// An off-chain process will be required to schedule the batch to run, but anyone can manually run
/// the batch.
///
/// Redeeming STAKE tokens requires NEAR to be unstaked and withdrawn from the staking pool.
/// When NEAR is unstaked, the unstaked NEAR funds are not available for withdrawal until 4 epochs
/// later (~2 days). While waiting for the unstaked NEAR funds to be released and withdrawn,
/// [run_redeem_stake_batch()](StakingService::run_redeem_stake_batch) requests will fail.
/// When a [RedeemStakeBatch](crate::interface::RedeemStakeBatch) is run, the STAKE
/// token value is computed at that point in time, which is used to compute the corresponding amount
/// of NEAR tokens to unstake from the staking pool. This information is recorded in a
/// [RedeemStakeBatchReceipt](crate::interface::RedeemStakeBatchReceipt), which is later used by user
/// accounts to claim NEAR tokens from the processed batch.
///
/// ## Notes
/// - Batches are processed serially
/// - Users can continue to submit requests to [deposit](StakingService::deposit) and [redeem](StakingService::redeem)
///   funds and they will be queued into the next batch
/// - batch receipts will be deleted from storage once all funds on the receipt are claimed
pub trait StakingService {
    /// returns the staking pool account ID used for the STAKE token
    /// - this is the staking pool that this contract is linked to
    fn staking_pool_id(&self) -> AccountId;

    /// looks up the receipt for the specified batch ID
    /// - when a batch is successfully processed a receipt is created, meaning the NEAR funds have
    ///   been successfully deposited and staked with the staking pool
    /// - the receipt is used by customer accounts to claim STAKE tokens for their staked NEAR based
    ///   on the STAKE token value at the point in time when the batch was run.
    /// - once all funds have been claimed from the receipt, then the receipt will be automatically
    ///   deleted from storage, i.e., if no receipt exists for the batch ID, then it means all funds
    ///   have been claimed (for valid batch IDs)
    fn stake_batch_receipt(&self, batch_id: BatchId) -> Option<StakeBatchReceipt>;

    /// looks up the receipt for the specified batch ID
    /// - when a batch is successfully processed a receipt is created, meaning the unstaked NEAR
    ///   has been withdrawn from the staking pool contract
    /// - the receipt is used by customer accounts to claim the unstaked NEAR tokens for their
    ///   redeemed STAKE tokens based on the STAKE token value at the point in time when the batch
    ///   was run
    /// - once all funds have been claimed from the receipt, then the receipt will be deleted from
    ///   storage, i.e., if no receipt exists for the batch ID, then it means all funds have been
    ///   claimed (for valid batch IDs)
    fn redeem_stake_batch_receipt(&self, batch_id: BatchId) -> Option<RedeemStakeBatchReceipt>;

    /// Adds the attached deposit to the next [StakeBatch] scheduled to run.
    /// Returns the [BatchId] for the [StakeBatch] that the funds are deposited into.
    /// - deposits are committed for staking via [run_stake_batch](StakingService::run_stake_batch)
    /// - each additional deposit request add the funds to the batch
    /// - NEAR funds can be withdrawn from the batch, as long as the batch is not yet committed via
    ///   - [withdraw_funds_from_stake_batch](StakingService::withdraw_funds_from_stake_batch)
    ///   - [withdraw_all_funds_from_stake_batch](StakingService::withdraw_all_funds_from_stake_batch)
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if no deposit is attached
    ///
    /// #\[payable\]
    fn deposit(&mut self) -> BatchId;

    /// withdraws specified amount from uncommitted stake batch and refunds the account
    ///
    /// NOTE: all batch receipts are first claimed
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are insufficient funds to fulfill the request
    /// - if the contract is locked
    fn withdraw_funds_from_stake_batch(&mut self, amount: YoctoNear);

    /// withdraws all NEAR from uncommitted stake batch and refunds the account
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
    /// ## Notes
    /// [contract_state](crates::interface::Operator::contract_state) can be queried to check if the
    /// batch cab be run, i.e., to check if there is a batch to run and that the contract is not locked.
    ///
    /// ## Panics
    /// - if contract is locked for
    ///   - staking batch is in progress
    ///   - unstaking is in progress
    /// - if there is no stake batch to run
    fn run_stake_batch(&mut self) -> Promise;

    /// Submits request to redeem STAKE tokens, which are put into a [RedeemStakeBatch](crate::interface::RedeemStakeBatch).
    /// The account's STAKE balance is debited the amount and moved into the batch.
    /// - redeem requests are committed via [run_redeem_stake_batch](StkaingService::run_redeem_stake_batch)
    /// - each redeem request adds the STAKE into the batch
    /// - the entire redeem batch can be cancelled via [cancel_uncommitted_redeem_stake_batch](StakingService::cancel_uncommitted_redeem_stake_batch)
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

    /// Redeems all available STAKE - see [redeem](StakingService::redeem)
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if the account has no STAKE to redeem
    fn redeem_all(&mut self) -> BatchId;

    /// Returns false if the account has no uncommitted redeem stake batch.
    /// - STAKE funds that were locked in the redeem stake batch are made available for transfer
    fn cancel_uncommitted_redeem_stake_batch(&mut self) -> bool;

    /// STAKE tokens are redeemed in 2 steps: first the corresponding NEAR is unstaked with the staking
    /// pool. Second, the NEAR funds need to be withdrawn from the staking pool. The unstaked NEAR
    /// funds are not immediately available. They are locked in the staking pool for 4 epochs. Further
    /// STAKE funds cannot be redeemed, i.e., unstaked from the staking pool, until the unstaked NEAR
    /// funds are withdrawn from the staking pool.
    ///
    /// For example, 50 NEAR are unstaked at epoch 100, which means the 50 NEAR is available
    /// for withdrawal at epoch 104. However, if a user submits a transaction to unstake another 50
    /// NEAR at epoch 103, then the entire 100 unstaked NEAR will be available to be withdrawn at
    /// epoch 107. In this example, in order to be able to withdraw the 50 NEAR at epoch 104, the 2nd
    /// unstaking request must be submitted after the NEAR is withdrawn.
    ///
    /// Thus, to redeem STAKE tokens, 2 sub-workflows are required: first workflow to unstake the NEAR
    /// following by a second workflow to withdraw the unstaked NEAR from the staking pool.
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
    /// ## Notes
    /// - [contract_state](crates::interface::Operator::contract_state) can be queried to check if the
    ///   batch cab be run, i.e., to check if there is a batch to run and that the contract is not locked.
    /// - while awaiting the unstaked NEAR funds to be withdrawn, NEAR funds can continue to be staked,
    ///   i.e., it is legal to invoke [run_stake_batch](StakingService::run_stake_batch)
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
