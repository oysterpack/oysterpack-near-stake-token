use crate::interface::{
    BatchId, RedeemStakeBatchReceipt, StakeBatchReceipt, StakeTokenValue, YoctoNear, YoctoStake,
};
use near_sdk::{json_types::ValidAccountId, AccountId, Promise, PromiseOrValue};

/// Integrates with the staking pool contract and manages STAKE token assets. The main use
/// cases supported by this interface are:
/// 1. Users can [deposit](StakingService::deposit) NEAR funds to stake.
/// 2. Users can withdraw NEAR funds from [StakeBatch](crate::interface::StakeBatch) that has not yet run.
/// 3. Once the NEAR funds are staked, the account is issued STAKE tokens based on the STAKE token
///    value computed when the [StakeBatch](crate::interface::StakeBatch) is run.
/// 4. Users can [redeem](StakingService::redeem) STAKE tokens for NEAR.
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

    /// Adds the attached deposit to the next [StakeBatch](crate::domain::StakeBatch) scheduled to run.
    /// Returns the [BatchId](crate::domain::BatchId) for the [StakeBatch](crate::domain::StakeBatch)
    /// that the funds are deposited into.
    /// - deposits are committed for staking via [stake](StakingService::stake)
    /// - each additional deposit request add the funds to the batch
    /// - NEAR funds can be withdrawn from the batch, as long as the batch is not yet committed via
    ///   - [withdraw_from_stake_batch](StakingService::withdraw_from_stake_batch)
    ///   - [withdraw_all_from_stake_batch](StakingService::withdraw_all_from_stake_batch)
    /// - a minimum deposit is required equivalent to 1000 yoctoSTAKE based on the most recent STAKE
    ///   token value
    ///   - this protects against the scenario of issuing zero STAKE tokens - we never want to issue
    ///     zero yoctoSTAKE tokens if NEAR is deposited and staked
    ///   - in addition because of rounding issues when
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if no deposit is attached
    /// - if less than the minimum required deposit was attached
    ///
    /// ## Notes
    /// - as a side effect, batch receipts are claimed
    ///
    /// #\[payable\]
    ///
    /// GAS REQUIREMENTS: 10 TGas
    fn deposit(&mut self) -> BatchId;

    /// If there is pending unstaked NEAR awaiting to become available for withdrawal, then the the
    /// NEAR deposits stored in the [StakeBatch](crate::domain::StakeBatch) will provide liquidity
    /// to enable NEAR funds to be withdrawn sooner than the lockup period imposed by the staking pool.
    /// When liquidity is added, instead of depositing funds into the staking pool, unstaked NEAR is
    /// simply restaked.
    ///
    /// locks the contract to stake the batched NEAR funds and then kicks off the staking workflow
    /// 1. lock the contract
    /// 2. get the account from the staking pool
    /// 3. if there is a pending withdrawal, then add liquidity
    ///    - if the amount being staked is less than the amount unstaked, then stake the batch amount
    ///      from the unstaked balance
    ///    - if the amount being staked is more than the unstaked amount, then deposit_and_stake the
    ///      remainder and then stake the batch amount
    /// 4. if there is no pending withdrawal, then deposits and stakes the NEAR funds with the staking pool
    /// 5. update STAKE token value
    /// 6. update liquidity and check if liquidity can clear pending withdrawal
    /// 7. create the batch receipt
    /// 8. release the lock
    ///
    /// ## Notes
    /// [contract_state](crate::interface::Operator::contract_state) can be queried to check if the
    /// batch cab be run, i.e., to check if there is a batch to run and that the contract is not locked.
    ///
    /// ## Panics
    /// - if contract is locked for
    ///   - staking batch is in progress
    ///   - unstaking is in progress
    /// - if there is no stake batch to run
    /// - if the attached deposit is less than the [minimum required deposit](StakingService::min_required_deposit_to_stake)
    ///
    /// GAS REQUIREMENTS: 200 TGas
    fn stake(&mut self) -> PromiseOrValue<BatchId>;

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
    /// GAS REQUIREMENTS: 225 TGas
    fn deposit_and_stake(&mut self) -> PromiseOrValue<BatchId>;

    /// withdraws specified amount from uncommitted stake batch and refunds the account
    ///
    /// NOTE: all batch receipts are first claimed
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are insufficient funds to fulfill the request
    /// - if the contract is locked
    fn withdraw_from_stake_batch(&mut self, amount: YoctoNear);

    /// withdraws all NEAR from uncommitted stake batch and refunds the account
    /// - returns NEAR amount that was withdrawn from the [StakeBatch](crate::domain::StakeBatch)
    ///
    /// NOTE: all batch receipts are first claimed
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if the contract is locked
    fn withdraw_all_from_stake_batch(&mut self) -> YoctoNear;

    /// Submits request to redeem STAKE tokens, which are put into a [RedeemStakeBatch](crate::interface::RedeemStakeBatch).
    /// In effect, this locks up STAKE in the [RedeemStakeBatch](crate::interface::RedeemStakeBatch),
    /// and the STAKE tokens are no longer tradeable.  
    /// The account's STAKE balance is debited the amount and moved into the batch.
    /// - redeem STAKE tokens are committed and unstaked via [unstake](StakingService::unstake)
    /// - each redeem request adds the STAKE into the batch
    /// - STAKE can be removed from uncommitted batches via:
    ///   - [remove_from_redeem_stake_batch](StakingService::remove_from_redeem_stake_batch)
    ///   - [remove_all_from_redeem_stake_batch](StakingService::remove_all_from_redeem_stake_batch)
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
    /// Returns None if there are no STAKE funds to redeem
    ///
    /// ## Panics
    /// - if account is not registered
    fn redeem_all(&mut self) -> Option<BatchId>;

    /// Enables the user to remove all STAKE that was redeemed and placed into the uncomitted
    /// [RedeemStakeBatch](crate::domain::RedeemStakeBatch). This effectively unlocks the STAKE
    /// that was specified to be redeemed.
    ///
    /// Returns the amount of STAKE that was unlocked.
    ///
    /// ## Panics
    /// - if the account is not registered
    fn remove_all_from_redeem_stake_batch(&mut self) -> YoctoStake;

    /// Enables the user to remove the specified amount of STAKE from the uncommitted [RedeemStakeBatch](crate::domain::RedeemStakeBatch)
    fn remove_from_redeem_stake_batch(&mut self, amount: YoctoStake);

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
    /// - [contract_state](crate::interface::Operator::contract_state) can be queried to check if the
    ///   batch cab be run, i.e., to check if there is a batch to run and that the contract is not locked.
    /// - while the unstake workflow is locked, users can continue to submit [redeem](StakingService::redeem)
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
    ///
    /// If there are no STAKE funds to redeem, then None is returned.
    /// If the contract is current locked, then the STAKE tokens are put into the next batch to redeem.
    /// Otherwise it proceeds with the unstake workflow and returns a Promise.
    ///
    /// GAS REQUIREMENTS: 150 TGas
    fn redeem_all_and_unstake(&mut self) -> PromiseOrValue<Option<BatchId>>;

    /// Returns the batch that is awaiting for funds to be available to be withdrawn.
    ///
    /// NOTE: pending withdrawals blocks [RedeemStakeBatch](crate::domain::RedeemStakeBatch) to run
    fn pending_withdrawal(&self) -> Option<RedeemStakeBatchReceipt>;

    /// Enables the user to claim receipts explicitly, which will also claim any available NEAR
    /// liquidity to settle [RedeemStakeBatchReceipts](crate::domain::RedeemStakeBatchReceipt) that
    /// have unstaked NEAR tokens locked in the staking pool and pending withdrawal
    ///
    /// ## Notes
    /// Receipts will also be claimed implicitly when the user submits any transactions.
    ///
    /// ## Panics
    /// if account is not registered
    fn claim_receipts(&mut self);

    /// Withdraws the specified amount from the account's available NEAR balance and transfers the
    /// funds to the account.
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are not enough available NEAR funds to fulfill the request
    fn withdraw(&mut self, amount: YoctoNear);

    /// Withdraws all available NEAR funds from the account and transfers the funds to the account.
    ///
    /// Returns the amount withdrawn.
    ///
    /// ## Panics
    /// - if the account is not registered
    fn withdraw_all(&mut self) -> YoctoNear;

    /// Transfers the specified amount from the account's available NEAR balance to the specified
    /// recipient account.
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are not enough available NEAR funds to fulfill the request
    fn transfer_near(&mut self, recipient: ValidAccountId, amount: YoctoNear);

    /// Transfers all available NEAR funds from the account's available NEAR balance to the specified
    /// recipient account.
    ///
    /// ## Panics
    /// - if the account is not registered
    fn transfer_all_near(&mut self, recipient: ValidAccountId) -> YoctoNear;

    /// In order to make sure STAKE tokens are issued when NEAR is staked, the user needs to deposit
    /// a minimum required amount based on the cached STAKE token value to issue ~100 yoctoSTAKE.
    ///
    /// NOTE: the min required deposit amount is conservative and the exact STAKE token value will
    /// only be known when the deposit is staked into the staking pool
    fn min_required_deposit_to_stake(&self) -> YoctoNear;

    /// The only reliable way to get an accurate STAKE token value is to lock the balances on the contract
    /// while retrieving the updated staking pool account balances. The cached STAKE token value is
    /// considered current if the lookup is within the same epoch period because staking rewards are
    /// only issued per epoch. To ensure that all staking pool rewards have been applied, then specify
    /// `refresh=true` which will always ping the staking pool contract and fetch balances.
    ///
    /// - If refresh is true, then the [`StakeTokenValue`] is always refreshed from the staking pool.
    /// - If refresh is not specified or false:
    ///   - if the epoch has changed for the cached [`StakeTokenValue`], then the [`StakeTokenValue`]
    ///     is refreshed from the staking pool.
    ///   - otherwise the cached STAKE token value is returned
    ///
    /// ### [`StakeTokenValue`] Refresh Workflow
    /// 1. Lock the contract to lock the balances while refreshing the STAKE token value
    /// 2. Submit as batch transaction to staking pool:
    ///    2.1 Ping the staking pool contract to distribute rewards
    ///    2.2 Get updated staking account balances from the staking pool
    /// 3. Update the cached [`StakeTokenValue`]
    /// 4. Unlock the contract
    ///
    /// ### Panics
    /// - if the contract is locked
    fn refresh_stake_token_value(&mut self) -> Promise;

    /// If the STAKE token value was last updated within the same epoch, then it is considered current
    /// and returned because staking rewards are distributed per epoch.
    ///
    /// Otherwise, the STAKE token value is considered stale and None is returned. This should signal
    /// the client to refresh the STAKE token value - [`refresh_stake_token_value`].
    ///
    /// ### NOTES
    /// The STAKE token value is refreshed each time the NEAR is staked and when STAKE is redeemed.
    fn stake_token_value(&self) -> Option<StakeTokenValue>;
}

pub mod events {
    use crate::domain::{self, BatchId, RedeemStakeBatchReceipt, StakeBatchReceipt};
    use crate::near::YOCTO;

    #[derive(Debug)]
    pub struct StakeTokenValue {
        pub total_staked_near_balance: u128,
        pub total_stake_supply: u128,
        /// the value of 1 STAKE token in NEAR
        pub stake_value: u128,
        /// blockchain point in time
        pub block_height: u64,
        pub block_timestamp: u64,
        pub epoch_height: u64,
    }

    impl From<domain::StakeTokenValue> for StakeTokenValue {
        fn from(value: domain::StakeTokenValue) -> Self {
            Self {
                total_staked_near_balance: value.total_staked_near_balance().value(),
                total_stake_supply: value.total_stake_supply().value(),
                stake_value: value.stake_to_near(YOCTO.into()).value(),
                block_height: value.block_time_height().block_height().value(),
                block_timestamp: value.block_time_height().block_timestamp().value(),
                epoch_height: value.block_time_height().epoch_height().value(),
            }
        }
    }

    #[derive(Debug)]
    pub struct Unstaked {
        /// corresponds to the [RedeemStakeBatch](crate::domain::RedeemStakeBatch)
        pub batch_id: u128,
        /// how much STAKE was redeemed in the batch
        pub stake: u128,
        /// how much NEAR was unstaked for the redeemed STAKE
        pub near: u128,
        /// STAKE token value used to compute amount of NEAR to unstake for redeemed STAKE tokens
        pub stake_token_value: StakeTokenValue,
    }

    impl Unstaked {
        pub fn new(batch_id: BatchId, receipt: &RedeemStakeBatchReceipt) -> Self {
            Self {
                batch_id: batch_id.value(),

                stake: receipt.redeemed_stake().value(),
                near: receipt.stake_near_value().value(),
                stake_token_value: receipt.stake_token_value().into(),
            }
        }
    }

    #[derive(Debug)]
    pub struct NearLiquidityAdded {
        /// how liquidity was added
        pub amount: u128,
        /// updated liquidity balance
        pub balance: u128,
    }

    #[derive(Debug)]
    pub struct Staked {
        /// corresponds to the [StakeBatch](crate::domain::StakeBatch)
        pub batch_id: u128,
        /// how much NEAR was staked
        pub near: u128,
        /// how much STAKE was minted for the staked NEAR
        pub stake: u128,
        /// STAKE token value used to mint new STAKE
        pub stake_token_value: StakeTokenValue,
    }

    impl Staked {
        pub fn new(batch_id: BatchId, receipt: &StakeBatchReceipt) -> Self {
            Self {
                batch_id: batch_id.value(),
                stake: receipt.near_stake_value().value(),
                near: receipt.staked_near().value(),
                stake_token_value: receipt.stake_token_value().into(),
            }
        }
    }

    #[derive(Debug)]
    pub struct PendingWithdrawalCleared {
        /// corresponds to the [RedeemStakeBatch](crate::domain::RedeemStakeBatch)
        pub batch_id: u128,
        /// how much STAKE was redeemed in the batch
        pub stake: u128,
        /// how much NEAR was unstaked for the redeemed STAKE
        pub near: u128,
        /// STAKE token value used to compute amount of NEAR to unstake for redeemed STAKE tokens
        pub stake_token_value: StakeTokenValue,
    }

    impl PendingWithdrawalCleared {
        pub fn new(batch: &domain::RedeemStakeBatch, receipt: &RedeemStakeBatchReceipt) -> Self {
            Self {
                batch_id: batch.id().value(),
                stake: batch.balance().amount().value(),
                near: receipt
                    .stake_token_value()
                    .stake_to_near(batch.balance().amount())
                    .value(),
                stake_token_value: receipt.stake_token_value().into(),
            }
        }
    }

    #[derive(Debug)]
    pub struct StakeBatch {
        /// corresponds to the [StakeBatch](crate::domain::StakeBatch)
        pub batch_id: u128,
        /// how much NEAR to staked is in the batch
        pub near: u128,
    }

    impl From<domain::StakeBatch> for StakeBatch {
        fn from(batch: domain::StakeBatch) -> Self {
            Self {
                batch_id: batch.id().value(),
                near: batch.balance().amount().value(),
            }
        }
    }

    /// batch is cancelled if all funds are withdrawn
    #[derive(Debug)]
    pub struct StakeBatchCancelled {
        pub batch_id: u128,
    }

    #[derive(Debug)]
    pub struct RedeemStakeBatch {
        /// corresponds to the [RedeemStakeBatch](crate::domain::RedeemStakeBatch)
        pub batch_id: u128,
        /// how much STAKE to redeem is in the batch
        pub stake: u128,
    }

    impl From<domain::RedeemStakeBatch> for RedeemStakeBatch {
        fn from(batch: domain::RedeemStakeBatch) -> Self {
            Self {
                batch_id: batch.id().value(),
                stake: batch.balance().amount().value(),
            }
        }
    }

    /// batch is cancelled if all funds are withdrawn
    #[derive(Debug)]
    pub struct RedeemStakeBatchCancelled {
        pub batch_id: u128,
    }

    #[cfg(test)]
    mod test {

        use super::*;
        use crate::domain::{RedeemStakeBatch, StakeTokenValue};
        use crate::near::YOCTO;
        use crate::test_utils::*;
        use near_sdk::{testing_env, MockedBlockchain};

        #[test]
        fn unstaked_near_log_fmt() {
            let account_id = "alfio-zappala.near";
            let context = new_context(account_id);
            testing_env!(context.clone());

            let batch = RedeemStakeBatch::new(1.into(), (10 * YOCTO).into());
            let receipt = batch.create_receipt(StakeTokenValue::default());
            let event = Unstaked::new(batch.id(), &receipt);
            println!("{:#?}", event);
        }
    }
}
