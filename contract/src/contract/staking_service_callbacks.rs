use crate::{
    domain, interface::StakeTokenValue, interface::StakingService,
    near::assert_predecessor_is_self, StakeTokenContract,
};
use near_sdk::json_types::U128;
use near_sdk::{env, near_bindgen, PromiseOrValue};

type Balance = U128;

#[near_bindgen]
impl StakeTokenContract {
    pub fn on_get_account_staked_balance(
        &self,
        #[callback] staked_balance: Balance,
    ) -> StakeTokenValue {
        assert_predecessor_is_self();
        assert!(
            self.promise_result_succeeded(),
            "failed to get staked balance from staking pool"
        );
        domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.amount()).into()
    }

    /// updates the cached [StakeTokenValue]
    pub fn on_refresh_account_staked_balance(
        &mut self,
        #[callback] staked_balance: Balance,
    ) -> StakeTokenValue {
        assert_predecessor_is_self();
        assert!(
            self.promise_result_succeeded(),
            "failed to get staked balance from staking pool"
        );
        self.stake_token_value =
            domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.amount());
        self.stake_token_value.into()
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::interface::Operator;
    use crate::{interface::AccountManagement, near::YOCTO, test_utils::*};
    use near_sdk::{serde_json, testing_env, MockedBlockchain};

    #[test]
    fn on_get_account_staked_balance_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        contract.total_stake.credit(YOCTO.into());
        let stake_token_value = contract.on_get_account_staked_balance(YOCTO.into());
        assert_eq!(
            stake_token_value.total_stake_supply,
            contract.total_stake.amount().into()
        );
        assert_eq!(stake_token_value.total_staked_near_balance, YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "failed to get staked balance from staking pool")]
    fn on_get_account_staked_balance_failure() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        // because of race conditions, this might pass, but eventually it will fail
        set_env_with_failed_promise_result(&mut contract);
        assert!(
            !contract.promise_result_succeeded(),
            "promise result should be failed"
        );
        contract.total_stake.credit(YOCTO.into());
        contract.on_get_account_staked_balance(YOCTO.into());
    }

    /// Given the promise ro get the staked balance completes successfully
    /// When the callback is invoked
    /// Then the StakeTokenValue cached value is updated
    /// And the batch funds are deposited and staked with the staking pool
    /// And a callback is scheduled to run once the deposit and stake promise completes
    #[test]
    fn on_run_stake_batch_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        let initial_stake_token_value = match contract.stake_token_value() {
            PromiseOrValue::Value(value) => value,
            _ => panic!("expected cached StakeTokenValue to be returned"),
        };

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        // account deposits into stake batch
        contract.deposit();
        contract.run_stake_batch();

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        context.attached_deposit = 100 * YOCTO;
        context.epoch_height += 1;
        testing_env!(context.clone());
        contract.on_run_stake_batch(0.into());
        let stake_token_value_after_callback = match contract.stake_token_value() {
            PromiseOrValue::Value(value) => value,
            _ => panic!("expected cached StakeTokenValue to be returned"),
        };
        assert!(
            stake_token_value_after_callback
                .block_time_height
                .epoch_height
                .value()
                > initial_stake_token_value
                    .block_time_height
                    .epoch_height
                    .value(),
            "stake token value should have been updated"
        );

        let receipts: Vec<Receipt> = env::created_receipts()
            .iter()
            .map(|receipt| {
                let json = serde_json::to_string_pretty(receipt).unwrap();
                println!("{}", json);
                let receipt: Receipt = serde_json::from_str(&json).unwrap();
                receipt
            })
            .collect();
        assert_eq!(receipts.len(), 2);

        // check `deposit_and_stake` func call action
        receipts
            .iter()
            .find(|receipt| {
                receipt.receiver_id == contract.staking_pool_id && {
                    match receipt.actions.first().unwrap() {
                        Action::FunctionCall {
                            method_name,
                            deposit,
                            gas,
                            ..
                        } => {
                            method_name == "deposit_and_stake"
                                && *deposit == context.attached_deposit
                                && *gas
                                    == contract
                                        .config
                                        .gas_config()
                                        .staking_pool()
                                        .deposit_and_stake()
                                        .value()
                        }
                        _ => false,
                    }
                }
            })
            .unwrap();

        // verify that `on_deposit_and_stake` callback is present
        receipts
            .iter()
            .find(|receipt| {
                receipt.receiver_id == context.current_account_id && {
                    match receipt.actions.first().unwrap() {
                        Action::FunctionCall {
                            method_name,
                            deposit,
                            gas: _,
                            ..
                        } => method_name == "on_deposit_and_stake" && *deposit == 0,
                        _ => false,
                    }
                }
            })
            .unwrap();
    }

    /// Given the promise result failed for getting the staked balance
    /// Then the callback fails
    #[test]
    #[should_panic(expected = "failed to get staked balance from staking pool")]
    fn on_run_stake_batch_promise_result_fails() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();
        contract.run_stake_batch();

        assert!(contract.run_stake_batch_locked);

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());
        set_env_with_failed_promise_result(&mut contract);
        contract.on_run_stake_batch(0.into());
    }

    /// Given the funds were successfully deposited and staked into the staking pool
    /// Then the stake batch receipts is saved
    /// And the total STAKE supply is updated
    /// And if there are funds in the next stake batch, then move it into the current batch
    #[test]
    fn run_stake_batch_workflow_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        {
            let staked_near_amount = 100 * YOCTO;
            context.attached_deposit = staked_near_amount;
            testing_env!(context.clone());
            contract.deposit();

            {
                context.attached_deposit = 0;
                testing_env!(context.clone());
                // capture the batch ID to lookup the batch receipt after the workflow is done
                let batch_id = contract.stake_batch.unwrap().id();
                contract.run_stake_batch();
                assert!(contract.run_stake_batch_locked);
                {
                    context.predecessor_account_id = context.current_account_id.clone();
                    testing_env!(context.clone());
                    contract.on_run_stake_batch(0.into()); // callback

                    {
                        context.predecessor_account_id = context.current_account_id.clone();
                        testing_env!(context.clone());
                        contract.on_deposit_and_stake(); // callback

                        let _receipt = contract.stake_batch_receipts.get(&batch_id).expect(
                            "receipt should have been created by `on_deposit_and_stake` callback",
                        );

                        assert_eq!(
                            contract.total_stake.amount(),
                            contract
                                .stake_token_value
                                .near_to_stake(staked_near_amount.into())
                        );

                        {
                            context.predecessor_account_id = context.current_account_id.clone();
                            testing_env!(context.clone());
                            contract.release_run_stake_batch_lock();
                            assert!(!contract.run_stake_batch_locked);
                        }
                    }
                }
            }
        }
    }
}
