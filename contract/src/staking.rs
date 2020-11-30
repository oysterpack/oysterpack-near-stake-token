use crate::common::{StakingPoolAccountId, YoctoNEAR, YoctoSTAKE};
use near_sdk::{
    serde::{self, Deserialize, Serialize},
    Promise,
};

pub trait StakingService {
    /// Deposits the attached amount into the predecessor account and stakes it with the specified
    /// staking pool contract. Once the funds are successfully staked, then STAKE tokens are issued
    /// to the predecessor account.
    ///
    /// ## Account Storage Fees
    /// - any applicable storage fees will be deducted from the attached deposit
    /// - when staking to a new staking pool, then storage fees will be charged
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if not enough deposit was attached to cover storage fees
    fn deposit_and_stake(&mut self, staking_pool_id: StakingPoolAccountId) -> Promise;
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeReceipt {
    /// amount in yoctoNEAR that was staked
    pub near_tokens_staked: YoctoNEAR,
    /// amount of yoctoSTAKE that was credited to the customer account
    pub stake_tokens_credit: YoctoSTAKE,
    /// amount of storage fees that were deducted
    ///
    /// ## Notes
    /// - storage fees will be charged when staking with a new staking pool
    pub storage_fees: Option<YoctoNEAR>,
}
