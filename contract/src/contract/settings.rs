use crate::{
    config::Config,
    core::Hash,
    domain::{
        Account, BatchId, BlockHeight, RedeemStakeBatch, RedeemStakeBatchReceipt, StakeBatch,
        StakeBatchReceipt, StakeTokenValue, StorageUsage, TimestampedNearBalance,
        TimestampedStakeBalance, YoctoNear, YoctoNearValue, YoctoStake,
    },
    near::storage_keys::{
        ACCOUNTS_KEY_PREFIX, REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX,
        STAKE_BATCH_RECEIPTS_KEY_PREFIX,
    },
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, UnorderedMap},
    env,
    json_types::ValidAccountId,
    near_bindgen,
    serde::{Deserialize, Serialize},
    wee_alloc, AccountId, PromiseResult,
};
use std::{
    convert::{TryFrom, TryInto},
    fmt::{self, Display, Formatter},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractSettings {
    pub staking_pool_id: ValidAccountId,
    pub config: Option<Config>,
    pub operator_id: ValidAccountId,
}

impl ContractSettings {
    /// depends on NEAR runtime env
    pub fn new(
        staking_pool_id: AccountId,
        operator_id: AccountId,
        config: Option<Config>,
    ) -> Result<Self, InvalidContractSettings> {
        let settings = Self {
            staking_pool_id: staking_pool_id
                .try_into()
                .map_err(|_| InvalidContractSettings::InvalidStakingPoolId)?,
            config,
            operator_id: operator_id
                .try_into()
                .map_err(|_| InvalidContractSettings::InvalidOperatorId)?,
        };

        match settings.validate() {
            Some(err) => Err(err),
            None => Ok(settings),
        }
    }

    pub fn validate(&self) -> Option<InvalidContractSettings> {
        if env::current_account_id().as_str() == self.operator_id.as_ref().as_str() {
            Some(InvalidContractSettings::OperatorMustNotBeContract)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum InvalidContractSettings {
    InvalidStakingPoolId,
    InvalidOperatorId,
    OperatorMustNotBeContract,
}

impl Display for InvalidContractSettings {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            InvalidContractSettings::OperatorMustNotBeContract => {
                write!(f, "operator account ID must not be the contract account ID")
            }
            InvalidContractSettings::InvalidOperatorId => write!(f, "invalid operator account ID"),
            InvalidContractSettings::InvalidStakingPoolId => {
                write!(f, "invalid staking pool account ID")
            }
        }
    }
}
