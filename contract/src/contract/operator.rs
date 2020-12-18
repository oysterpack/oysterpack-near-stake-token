use crate::{
    contract::ext_staking_pool, domain::RedeemLock, interface::Operator, near::NO_DEPOSIT,
    StakeTokenContract,
};

use near_sdk::{env, near_bindgen, Promise};

#[near_bindgen]
impl Operator for StakeTokenContract {
    fn release_run_stake_batch_lock(&mut self) {
        self.assert_predecessor_is_self_or_operator();
        self.run_stake_batch_locked = false;
    }

    fn release_run_redeem_stake_batch_unstaking_lock(&mut self) {
        self.assert_predecessor_is_self_or_operator();

        if let Some(RedeemLock::Unstaking) = self.run_redeem_stake_batch_lock {
            self.run_redeem_stake_batch_lock = None
        }
    }

    fn withdraw_all_funds_from_staking_pool(&self) -> Promise {
        self.assert_predecessor_is_self_or_operator();

        ext_staking_pool::withdraw_all(
            &self.staking_pool_id,
            NO_DEPOSIT.into(),
            self.config.gas_config().staking_pool().withdraw().value(),
        )
    }
}
