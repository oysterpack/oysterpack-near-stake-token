use crate::{contract::ext_staking_pool, interface::Operator, StakeTokenContract};

use crate::near::NO_DEPOSIT;
use near_sdk::{env, near_bindgen, Promise};

#[near_bindgen]
impl Operator for StakeTokenContract {
    fn release_run_stake_batch_lock(&mut self) {
        self.assert_predecessor_is_self_or_operator();
        self.run_stake_batch_locked = false;
    }

    fn withdraw_all_funds_from_staking_pool(&self) -> Promise {
        ext_staking_pool::get_account_unstaked_balance(
            env::current_account_id(),
            &self.staking_pool_id,
            NO_DEPOSIT.into(),
            self.config.gas_config().staking_pool().withdraw().value(),
        )
    }
}
