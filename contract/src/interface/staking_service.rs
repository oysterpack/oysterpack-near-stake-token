use crate::interface::{
    BatchId, RedeemStakeBatchReceipt, StakeBatchReceipt, YoctoNear, YoctoStake,
};
use near_sdk::{AccountId, Promise, PromiseOrValue};

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
/// The staking batch workflow is run via [stake()](StakingService::stake), which deposits and stakes
/// the batched NEAR funds with the staking pool. Users have the option to combine the deposit and
/// stake operations via [deposit_and_stake()](StakingService::deposit_and_stake).
///
/// When the batch is run, the STAKE token value at that point in time is computed and recorded into a
/// [StakeBatchReceipt](crate::interface::StakeBatchReceipt). The STAKE tokens are issued on demand when
/// the user accounts access the contract for actions that involve STAKE tokens - when staking NEAR,
/// redeeming STAKE, or withdrawing unstaked NEAR.
///
/// ## How Redeeming STAKE Works
/// Users [redeem](StakingService::redeem) STAKE tokens, which are collected into a
/// [RedeemStakeBatch](crate::interface::RedeemStakeBatch). The redeemed STAKE tokens will need to be
/// unstaked from the staking pool via [unstake()](StakingService::unstake), which processes the
/// [RedeemStakeBatch](crate::interface::RedeemStakeBatch). Users have the option to combine the
/// redeem and unstake operations via [redeem_and_stake()](StakingService::redeem_and_unstake). For
/// convenience users can also simply redeem all STAKE via [redeem_all()](StakingService::redeem) and
/// [redeem_all_and_stake()](StakingService::redeem_all_and_unstake).
///
/// Redeeming STAKE tokens requires NEAR to be unstaked and withdrawn from the staking pool.
/// When NEAR is unstaked, the unstaked NEAR funds are not available for withdrawal until 4 epochs
/// later (~2 days). While waiting for the unstaked NEAR funds to be released and withdrawn,
/// [unstake()](StakingService::unstake) requests will fail.
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
    /// - deposits are committed for staking via [stake](StakingService::stake)
    /// - each additional deposit request add the funds to the batch
    /// - NEAR funds can be withdrawn from the batch, as long as the batch is not yet committed via
    ///   - [withdraw_funds_from_stake_batch](StakingService::withdraw_funds_from_stake_batch)
    ///   - [withdraw_all_funds_from_stake_batch](StakingService::withdraw_all_funds_from_stake_batch)
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if no deposit is attached
    ///
    /// ## Notes
    /// Users who have pending redeem STAKE requests claim the NEAR from the liquidity pool in the
    /// following ways:
    /// 1. explicitly via [claim_near](StakingService::claim_near)
    /// 2. implicitly when withdrawing NEAR funds
    ///    - [withdraw_all](crate::interface::AccountManagement::withdraw_all)
    ///    - [withdraw](crate::interface::AccountManagement::withdraw_all)
    /// 3. implicitly when the user who is depositing has pending redeem stake batches
    ///
    /// #\[payable\]
    ///
    /// GAS REQUIREMENTS: 10 TGas
    fn deposit(&mut self) -> BatchId;

    /// If there is pending unstaked NEAR awaiting to become available for withdrawal, then the the
    /// NEAR deposits stored in the [StakeBatch](crate::domain::StakeBatch] will provide liquidity
    /// to enable NEAR funds to be withdrawn sooner than the lockup period imposed by the staking pool.
    /// When liquidity is added, instead of depositing funds into the staking pool, unstaked NEAR is
    /// simply restaked.
    ///
    /// locks the contract to stake the batched NEAR funds and then kicks off the staking workflow
    /// 1. lock the contract
    /// 2. gets the account stake balance from the staking pool
    /// 3. updates STAKE token value
    /// 4. if there is a pending withdrawal, then add liquidity
    ///    - if the amount being staked is less than the amount unstaked, then stake the batch amount
    ///      from the unstaked balance
    ///    - if the amount being staked is more than the unstaked amount, then deposit_and_stake the
    ///      remainder and then stake the batch amount
    /// 5. if there is not pending withdrawal, then deposits and stakes the NEAR funds with the staking pool
    /// 6. creates the batch receipt
    /// 7. releases the lock
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
    ///
    /// GAS REQUIREMENTS: 150 TGas
    fn stake(&mut self) -> Promise;

    /// Combines [deposit](StakingService::deposit) and [stake](StakingService::stake) calls together.
    ///
    /// If the contract is currently locked, then the deposit cannot be be immediately staked. If the
    /// funds can be staked, then the staking Promise is returned. Otherwise, the funds are simply
    /// deposited into the next available batch and the batch ID is returned.
    ///
    /// ## Notes
    /// - the NEAR funds are committed into the stake batch before kicking off the [stake](StakingService::stake)
    ///   workflow. This means if the [stake](StakingService::stake) Promise fails, the NEAR funds
    ///   remain in the stake batch, and will be staked the next time [stake](StakingService::stake)
    /// - the [stake](StakingService::stake) workflow may fail if not enough gas was supplied to the
    ///   for the `deposit_and_stake` call on the staking pool - check the gas config
    ///
    /// #\[payable\]
    ///
    /// GAS REQUIREMENTS: 150 TGas
    fn deposit_and_stake(&mut self) -> PromiseOrValue<BatchId>;

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

    /// Submits request to redeem STAKE tokens, which are put into a [RedeemStakeBatch](crate::interface::RedeemStakeBatch).
    /// In effect, this locks up STAKE in the [RedeemStakeBatch](crate::interface::RedeemStakeBatch),
    /// and the STAKE tokens are no longer tradeable.  
    /// The account's STAKE balance is debited the amount and moved into the batch.
    /// - redeem STAKE tokens are committed and unstaked via [unstake](StakingService::unstake)
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

    /// Runs the workflow to process redeem STAKE for NEAR from the staking pool. The workflow consists
    /// of 2 sub-workflows:
    /// 1. NEAR funds are unstaked with the staking pool
    /// 2. NEAR funds are withdrawn from the staking pool, once the unstaked NEAR funds become available
    ///    for withdrawal (4 epochs / ~2 days)
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
    /// - while the unstake workflow is locked, users can continue to submit [redeem](STakingService::redeem)
    ///   requests which will be run in the next batch
    /// - while awaiting the unstaked NEAR funds to be withdrawn, NEAR funds can continue to be staked,
    ///   i.e., it is legal to invoke [stake](StakingService::stake)
    /// - because unstaked NEAR funds are locked for 4 epochs, depending on unstake workflows that are
    ///   in progress, it may take a user 4-8 epochs to get access to their NEAR tokens for the STAKE
    ///   tokens they have redeemed. For example, user-1 unstakes at epoch 100, which means the next
    ///   unstaking is not eligible until epoch 104. If user-2 redeems STAKE in epoch 100, but after
    ///   the unstake workflow was run, then user-2 will need to wait until epoch 104 to run the unstake
    ///   workflow.
    ///
    /// ## Panics
    /// - if staking is in progress
    /// - if the redeem stake batch is already in progress
    /// - if pending withdrawal and unstaked funds are not available for withdrawal
    ///
    /// ## FAQ
    /// ### Why are the unstaked NEAR funds locked for 2 days?
    /// Because that is how the current [staking pools](https://github.com/near/core-contracts/tree/master/staking-pool)
    /// are designed to work.
    ///
    /// For example, 50 NEAR are unstaked at epoch 100, which means the 50 NEAR is available
    /// for withdrawal at epoch 104. However, if a user submits a transaction to unstake another 50
    /// NEAR at epoch 103, then the entire 100 unstaked NEAR will be available to be withdrawn at
    /// epoch 107. In this example, in order to be able to withdraw the 50 NEAR at epoch 104, the 2nd
    /// unstaking request must be submitted after the NEAR is withdrawn.
    ///
    /// GAS REQUIREMENTS: 150 TGas
    fn unstake(&mut self) -> Promise;

    /// combines the [redeem](StakingService::redeem) and [unstake](StakingService::unstake) calls
    ///
    /// GAS REQUIREMENTS: 150 TGas
    fn redeem_and_unstake(&mut self, amount: YoctoStake) -> PromiseOrValue<BatchId>;

    /// combines the [redeem_all](StakingService::redeem) and [unstake](StakingService::unstake) calls
    fn redeem_all_and_unstake(&mut self) -> PromiseOrValue<BatchId>;

    /// Returns the batch that is awaiting for funds to be available to be withdrawn.
    ///
    /// NOTE: pending withdrawals blocks [RedeemStakeBatch] to run
    fn pending_withdrawal(&self) -> Option<RedeemStakeBatchReceipt>;
}
