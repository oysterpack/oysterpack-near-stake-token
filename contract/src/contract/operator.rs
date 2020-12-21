//required in order for near_bindgen macro to work outside of lib.rs
use crate::*;
use crate::{
    contract::ext_staking_pool, domain::RedeemLock, interface::Operator, near::NO_DEPOSIT,
};

use near_sdk::{near_bindgen, Promise};

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

#[cfg(test)]
mod test {
    use super::*;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    fn release_run_redeem_stake_batch_unstaking_lock_with_unstaking_lock() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());
        contract.release_run_redeem_stake_batch_unstaking_lock();

        assert!(contract.run_redeem_stake_batch_lock.is_none());
    }

    #[test]
    fn release_run_redeem_stake_batch_unstaking_lock_invoked_by_operator() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        context.predecessor_account_id = contract.operator_id.clone();
        testing_env!(context.clone());
        contract.release_run_redeem_stake_batch_unstaking_lock();

        assert!(contract.run_redeem_stake_batch_lock.is_none());
    }

    #[test]
    fn release_run_redeem_stake_batch_unstaking_lock_with_pending_withdrawal_lock() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());
        contract.release_run_redeem_stake_batch_unstaking_lock();

        assert_eq!(
            contract.run_redeem_stake_batch_lock,
            Some(RedeemLock::PendingWithdrawal)
        );
    }

    #[test]
    #[should_panic(expected = "func call is pnly allowed internally or by an operator account")]
    fn release_run_redeem_stake_batch_unstaking_lock_access_denied() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.release_run_redeem_stake_batch_unstaking_lock();
    }
}
