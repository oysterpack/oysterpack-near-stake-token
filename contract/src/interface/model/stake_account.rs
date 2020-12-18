use crate::domain::Account;
use crate::interface::{
    RedeemStakeBatch, StakeBatch, TimestampedNearBalance, TimestampedStakeBalance,
};
use near_sdk::{
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeAccount {
    pub storage_escrow: TimestampedNearBalance,

    pub near: Option<TimestampedNearBalance>,
    pub stake: Option<TimestampedStakeBalance>,

    pub stake_batch: Option<StakeBatch>,
    pub next_stake_batch: Option<StakeBatch>,

    pub redeem_stake_batch: Option<RedeemStakeBatch>,
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
