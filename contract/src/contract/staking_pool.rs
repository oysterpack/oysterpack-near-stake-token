use crate::config::Config;
use crate::domain::YoctoNear;
use crate::near::NO_DEPOSIT;
use crate::StakeTokenContract;
use near_sdk::{
    env,
    json_types::U128,
    serde::{Deserialize, Serialize},
    serde_json, AccountId, Promise,
};

pub struct StakingPoolPromiseBuilder<'a>(Promise, &'a Config);

const NO_ARGS: [u8; 0] = [];

impl<'a> StakingPoolPromiseBuilder<'a> {
    pub fn new(account_id: AccountId, config: &'a Config) -> Self {
        Self(Promise::new(account_id), config)
    }

    pub fn promise(self) -> Promise {
        self.0
    }

    pub fn ping(self) -> Self {
        Self(
            self.0.function_call(
                b"ping".to_vec(),
                NO_ARGS.to_vec(),
                NO_DEPOSIT.into(),
                self.1.gas_config().staking_pool().ping().value(),
            ),
            self.1,
        )
    }

    pub fn get_account(self) -> Self {
        Self(
            self.0.function_call(
                b"get_account".to_vec(),
                serde_json::to_vec(&GetAccountArgs::default()).unwrap(),
                NO_DEPOSIT.into(),
                self.1.gas_config().staking_pool().get_account().value(),
            ),
            self.1,
        )
    }

    pub fn deposit_then_stake(self, deposit_amount: YoctoNear, stake_amount: YoctoNear) -> Self {
        Self(
            self.0
                .function_call(
                    b"deposit".to_vec(),
                    NO_ARGS.to_vec(),
                    deposit_amount.into(),
                    self.1.gas_config().staking_pool().deposit().value(),
                )
                .function_call(
                    b"stake".to_vec(),
                    serde_json::to_vec(&StakeArgs::from(stake_amount)).unwrap(),
                    NO_DEPOSIT.into(),
                    self.1.gas_config().staking_pool().stake().value(),
                ),
            self.1,
        )
    }

    pub fn stake(self, amount: YoctoNear) -> Self {
        Self(
            self.0.function_call(
                b"stake".to_vec(),
                serde_json::to_vec(&StakeArgs::from(amount)).unwrap(),
                NO_DEPOSIT.into(),
                self.1.gas_config().staking_pool().stake().value(),
            ),
            self.1,
        )
    }

    pub fn deposit_and_stake(self, amount: YoctoNear) -> Self {
        Self(
            self.0.function_call(
                b"deposit_and_stake".to_vec(),
                NO_ARGS.to_vec(),
                amount.into(),
                self.1
                    .gas_config()
                    .staking_pool()
                    .deposit_and_stake()
                    .value(),
            ),
            self.1,
        )
    }

    pub fn withdraw_all(self) -> Self {
        Self(
            self.0.function_call(
                b"withdraw_all".to_vec(),
                NO_ARGS.to_vec(),
                NO_DEPOSIT.into(),
                self.1.gas_config().staking_pool().withdraw().value(),
            ),
            self.1,
        )
    }

    pub fn unstake(self, amount: YoctoNear) -> Self {
        Self(
            self.0.function_call(
                b"unstake".to_vec(),
                serde_json::to_vec(&UnStakeArgs::from(amount)).unwrap(),
                NO_DEPOSIT.into(),
                self.1.gas_config().staking_pool().unstake().value(),
            ),
            self.1,
        )
    }

    pub fn unstake_all(self) -> Self {
        Self(
            self.0.function_call(
                b"unstake_all".to_vec(),
                NO_ARGS.to_vec(),
                NO_DEPOSIT.into(),
                self.1.gas_config().staking_pool().unstake().value(),
            ),
            self.1,
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct GetAccountArgs {
    pub account_id: AccountId,
}

impl Default for GetAccountArgs {
    fn default() -> Self {
        Self {
            account_id: env::current_account_id(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeArgs {
    pub amount: U128,
}

impl From<YoctoNear> for StakeArgs {
    fn from(amount: YoctoNear) -> Self {
        Self {
            amount: amount.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct UnStakeArgs {
    pub amount: U128,
}

impl From<YoctoNear> for UnStakeArgs {
    fn from(amount: YoctoNear) -> Self {
        Self {
            amount: amount.into(),
        }
    }
}

impl StakeTokenContract {
    pub(crate) fn staking_pool_promise(&self) -> StakingPoolPromiseBuilder {
        StakingPoolPromiseBuilder::new(self.staking_pool_id.clone(), &self.config)
    }
}
