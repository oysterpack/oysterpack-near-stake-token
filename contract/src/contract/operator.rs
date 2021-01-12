//required in order for near_bindgen macro to work outside of lib.rs
use crate::interface::ContractOwner;
use crate::*;
use crate::{
    domain::RedeemLock,
    interface::{contract_state::ContractState, AccountManagement},
    interface::{Operator, StakingService},
};
use near_sdk::{near_bindgen, Promise};

#[near_bindgen]
impl Operator for StakeTokenContract {
    fn contract_state(&self) -> ContractState {
        ContractState {
            block: domain::BlockTimeHeight::from_env().into(),
            config_change_block_height: self.config_change_block_height.into(),
            staking_pool_id: self.staking_pool_id.clone(),
            registered_accounts_count: self.total_registered_accounts().clone(),
            total_unstaked_near: self.total_near.into(),
            total_stake_supply: self.total_stake.into(),
            near_liquidity_pool: self.near_liquidity_pool.into(),
            stake_token_value: self.stake_token_value.into(),
            batch_id_sequence: self.batch_id_sequence.into(),
            stake_batch: self.stake_batch.map(interface::StakeBatch::from),
            next_stake_batch: self.next_stake_batch.map(interface::StakeBatch::from),
            redeem_stake_batch: self.redeem_stake_batch.map(|batch| {
                interface::RedeemStakeBatch::from(
                    batch,
                    self.redeem_stake_batch_receipt(batch.id().into()),
                )
            }),
            next_redeem_stake_batch: self.next_redeem_stake_batch.map(|batch| {
                interface::RedeemStakeBatch::from(
                    batch,
                    self.redeem_stake_batch_receipt(batch.id().into()),
                )
            }),
            pending_withdrawal: self.pending_withdrawal(),
            run_stake_batch_locked: self.run_stake_batch_locked,
            run_redeem_stake_batch_lock: self.run_redeem_stake_batch_lock,
            owner_available_balance: self.owner_balance(),
            initial_storage_usage: self.initial_contract_storage_usage.into(),
            storage_usage_growth: (env::storage_usage()
                - self.initial_contract_storage_usage.value())
            .into(),
        }
    }

    fn config(&self) -> interface::Config {
        self.config.into()
    }

    fn reset_config_default(&mut self) -> interface::Config {
        self.assert_predecessor_is_operator();
        self.config = Config::default();
        self.config.into()
    }

    fn update_config(&mut self, config: interface::Config) -> interface::Config {
        self.assert_predecessor_is_operator();
        self.config.merge(config);
        self.config_change_block_height = env::block_index().into();
        self.config.into()
    }

    fn force_update_config(&mut self, config: interface::Config) -> interface::Config {
        self.assert_predecessor_is_operator();
        self.config.force_merge(config);
        self.config_change_block_height = env::block_index().into();
        self.config.into()
    }

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
        self.staking_pool_promise().withdraw_all().promise()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{serde_json, testing_env, MockedBlockchain};

    #[test]
    fn release_run_redeem_stake_batch_unstaking_lock_with_unstaking_lock() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
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
        let mut contract = StakeTokenContract::new(None, contract_settings);
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
        let mut contract = StakeTokenContract::new(None, contract_settings);
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
    #[should_panic(expected = "contract call is only allowed internally or by an operator account")]
    fn release_run_redeem_stake_batch_unstaking_lock_access_denied() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.release_run_redeem_stake_batch_unstaking_lock();
    }

    #[test]
    fn contract_state_invoked_by_operator() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(None, contract_settings);
        const CONTRACT_STATE_STORAGE_OVERHEAD: u64 = 45;
        context.storage_usage +=
            contract.try_to_vec().unwrap().len() as u64 + CONTRACT_STATE_STORAGE_OVERHEAD;

        context.predecessor_account_id = contract.operator_id.clone();
        testing_env!(context.clone());
        let state = contract.contract_state();
        println!("{}", serde_json::to_string_pretty(&state).unwrap());
    }
}
