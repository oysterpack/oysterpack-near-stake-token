use crate::{interface::Operator, StakeTokenContract};

use near_sdk::near_bindgen;

#[near_bindgen]
impl Operator for StakeTokenContract {
    fn release_run_stake_batch_lock(&mut self) {
        self.assert_predecessor_is_self_or_operator();
        self.run_stake_batch_locked = false;
    }
}
