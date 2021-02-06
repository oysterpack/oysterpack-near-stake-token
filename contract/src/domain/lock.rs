use crate::domain::YoctoNear;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

#[derive(
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
)]
#[serde(crate = "near_sdk::serde")]
pub enum RedeemLock {
    Unstaking,
    /// while locked on pending withdrawal of unstaked funds, the receipt for the specified
    /// batch ID cannot be claimed
    PendingWithdrawal,
}

/// [`StakeLock::Staking`] -> [`StakeLock::Staked`] -> DONE
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum StakeLock {
    /// requests have been submitted to staking pool to deposit and stake funds
    /// - is triggered by [StakingService::stake()](crate::interface::StakingService::stake)
    /// - depositing and staking NEAR funds is performed as atomic batched transaction, i.e., if
    ///   the batched transaction fails for any reason, e.g., exceeded prepaid gas, then no funds
    ///   are transferred
    Staking,
    /// indicates the batch funds have been successfully staked with the staking pool, but the staked
    /// batch is not yet processed, i.e., balances need to be updated
    /// - stores the information needed to process the staked batch
    Staked {
        near_liquidity: Option<YoctoNear>,
        staked_balance: YoctoNear,
        unstaked_balance: YoctoNear,
    },
    /// balances need to be locked while refreshing STAKE token value
    RefreshingStakeTokenValue,
}
