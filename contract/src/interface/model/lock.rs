use crate::{domain, interface::YoctoNear};

use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
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
}

impl From<domain::StakeLock> for StakeLock {
    fn from(lock: domain::StakeLock) -> Self {
        match lock {
            domain::StakeLock::Staking => StakeLock::Staking,
            domain::StakeLock::Staked {
                near_liquidity,
                staked_balance,
                unstaked_balance,
            } => StakeLock::Staked {
                near_liquidity: near_liquidity.map(Into::into),
                staked_balance: staked_balance.into(),
                unstaked_balance: unstaked_balance.into(),
            },
        }
    }
}
