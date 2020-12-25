use crate::domain::Account;
use crate::interface::{
    RedeemStakeBatch, StakeBatch, TimestampedNearBalance, TimestampedStakeBalance,
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
}

impl From<Account> for StakeAccount {
    fn from(account: Account) -> Self {
        Self {
            storage_escrow: account.storage_escrow.into(),
            near: account.near.map(Into::into),
            stake: account.stake.map(Into::into),
            stake_batch: account.stake_batch.map(Into::into),
            next_stake_batch: account.next_stake_batch.map(Into::into),
            redeem_stake_batch: account.redeem_stake_batch.map(Into::into),
            next_redeem_stake_batch: account.next_redeem_stake_batch.map(Into::into),
        }
    }
}
