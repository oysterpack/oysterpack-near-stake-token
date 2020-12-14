use crate::{interface::Operator, StakeTokenContract};

use near_sdk::near_bindgen;

#[near_bindgen]
impl Operator for StakeTokenContract {
    fn unlock(&mut self) {
        self.assert_predecessor_is_self_or_operator();
        self.locked = false;
    }
}
