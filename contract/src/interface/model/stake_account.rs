use crate::interface::{
    RedeemStakeBatch, StakeBatch, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear,
};
use near_sdk::serde::{Deserialize, Serialize};

/// View model for a registered account with the contract
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeAccount {
    /// account storage usage payment that is escrowed
    /// - the balance will be refunded when the account unregisters
    /// - timestamp also shows when the account registered
    pub storage_escrow: TimestampedNearBalance,

    /// NEAR balance that is available for withdrawal from the contract
    pub near: Option<TimestampedNearBalance>,
    /// account STAKE token balance
    pub stake: Option<TimestampedStakeBalance>,

    /// NEAR funds that have been deposited to be staked when the batch is run
    pub stake_batch: Option<StakeBatch>,
    /// While batches are running, the contract is locked. The account can still deposit NEAR funds
    /// to stake into the next batch while the contract is locked.
    pub next_stake_batch: Option<StakeBatch>,

    /// STAKE tokens that have been set aside to be redeemed in the next batch
    pub redeem_stake_batch: Option<RedeemStakeBatch>,
    /// While batches are running, the contract is locked. The account can still set submit requests
    /// to redeem STAKE tokens into the next batch while the contract is locked.
    pub next_redeem_stake_batch: Option<RedeemStakeBatch>,

    /// only applies if the account has a [RedeemStakeBatch](crate::domain::RedeemStakeBatch) with a
    /// [RedeemStakeBatchReceipt](crate::domain::RedeemStakeBatchReceipt) that is pending withdrawal
    /// from the staking pool. If the contract has liquidity, then this returns the current liquidity
    /// that is available to withdraw against the redeemed STAKE. The account is not guaranteed the
    /// funds because other accounts might have withdrawn them first.
    ///
    /// returns None if there is currently no NEAR liquidity to withdraw against
    pub contract_near_liquidity: Option<YoctoNear>,
}
