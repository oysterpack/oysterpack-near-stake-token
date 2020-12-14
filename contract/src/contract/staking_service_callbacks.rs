use crate::{
    core::Hash,
    domain,
    domain::{Account, StakeBatch},
    ext_staking_pool, ext_staking_pool_callbacks,
    interface::{BatchId, RedeemStakeBatchReceipt, StakingService, YoctoStake},
    interface::{StakeBatchReceipt, StakeTokenValue, YoctoNear},
    near::{assert_predecessor_is_self, NO_DEPOSIT},
    RunStakeBatchFailure, StakeTokenContract,
};
use near_sdk::json_types::{U128, U64};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Promise, PromiseOrValue};

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
        domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.balance()).into()
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
            domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.balance());
        self.stake_token_value.into()
    }

    /// part of the [run_stake_batch] workflow
    pub fn on_get_account_staked_balance_to_run_stake_batch(
        &mut self,
        #[callback] staked_balance: Balance,
    ) -> PromiseOrValue<Result<StakeBatchReceipt, RunStakeBatchFailure>> {
        assert_predecessor_is_self();

        // the batch should always be present because the purpose of this callback is a step
        // in the batch processing workflow
        // - if the callback was called by itself, and the batch is not present, then there is a bug
        let batch = self.stake_batch.expect("stake batch must be present");

        if !self.promise_result_succeeded() {
            self.locked = false;
            return PromiseOrValue::Value(Err(RunStakeBatchFailure::GetStakedBalanceFailure(
                batch.id().into(),
            )));
        }

        // update the cached STAKE token value
        self.stake_token_value =
            domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.balance());

        let deposit_and_stake_gas = self
            .config
            .gas_config()
            .staking_pool()
            .deposit_and_stake()
            .value();
        let gas_needed_to_complete_this_func_call = self
            .config
            .gas_config()
            .on_get_account_staked_balance_to_run_stake_batch()
            .value();
        // give the remainder of the gas to the callback
        let callback_gas = env::prepaid_gas()
            - env::used_gas()
            - deposit_and_stake_gas
            - gas_needed_to_complete_this_func_call;

        ext_staking_pool::deposit_and_stake(
            &self.staking_pool_id,
            batch.balance().balance().value(),
            deposit_and_stake_gas,
        )
        .then(ext_staking_pool_callbacks::on_deposit_and_stake(
            staked_balance.0,
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            callback_gas,
        ))
        .into()
    }

    /// This is the last step in the [StakeBatch] run.
    ///
    /// NOTE: if this transaction fails, then the contract will remain stuck in a locked state
    /// TODO: how to recover if this fails and the contract remains locked ?
    pub fn on_deposit_and_stake(
        &mut self,
        staked_balance: Balance,
    ) -> Result<StakeBatchReceipt, RunStakeBatchFailure> {
        assert_predecessor_is_self();
        assert!(
            self.stake_batch.is_some(),
            "callback should only be invoked when there is a StakeBatch being processed"
        );

        let deposit_and_stake_succeeded = self.promise_result_succeeded();
        let result = if deposit_and_stake_succeeded {
            let batch = self.stake_batch.take().unwrap();

            // create batch receipt
            let stake_token_value =
                domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.balance());
            let stake_batch_receipt =
                domain::StakeBatchReceipt::new(batch.balance().balance(), stake_token_value);
            self.stake_batch_receipts
                .insert(&batch.id(), &stake_batch_receipt);

            // update the total STAKE supply
            self.total_stake
                .credit(stake_token_value.near_to_stake(batch.balance().balance()));

            // move the next batch into the current batch
            self.stake_batch = self.next_stake_batch.take();

            Ok(StakeBatchReceipt::new(batch.id(), stake_batch_receipt))
        } else {
            let batch = self.stake_batch.unwrap();
            env::log(format!("ERR: failed to process stake batch #{} - `deposit_and_stake` func call on staking pool failed", batch.id().value()).as_bytes());
            Err(RunStakeBatchFailure::DepositAndStakeFailure(
                batch.id().into(),
            ))
        };

        self.locked = false;
        result
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::{
        config::Config,
        domain::StakeBatchReceipt,
        interface::AccountManagement,
        near::{self, YOCTO},
        test_utils::*,
    };
    use near_sdk::{
        json_types::ValidAccountId, serde_json, testing_env, AccountId, MockedBlockchain, VMContext,
    };
    use std::convert::TryFrom;

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
            contract.total_stake.balance().into()
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
    fn on_get_account_staked_balance_to_run_stake_batch_success() {
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
        match contract.on_get_account_staked_balance_to_run_stake_batch(0.into()) {
            PromiseOrValue::Value(result) => panic!("expecting promise"),
            PromiseOrValue::Promise(_) => {}
        }
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
                            gas,
                            ..
                        } => method_name == "on_deposit_and_stake" && *deposit == 0,
                        _ => false,
                    }
                }
            })
            .unwrap();
    }

    /// Given the promise result failed for getting the staked balance
    /// Then the contract is unlocked
    /// And the callback returns a GetStakedBalanceFailure failure result
    #[test]
    fn on_get_account_staked_balance_to_run_stake_batch_promise_result_fails() {
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

        assert!(contract.locked);

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        context.attached_deposit = 100 * YOCTO;
        context.epoch_height += 1;
        testing_env!(context.clone());
        set_env_with_failed_promise_result(&mut contract);
        match contract.on_get_account_staked_balance_to_run_stake_batch(0.into()) {
            PromiseOrValue::Value(Err(RunStakeBatchFailure::GetStakedBalanceFailure(
                bactch_id,
            ))) => {
                assert_eq!(contract.stake_batch.unwrap().id().value(), bactch_id.into());
                assert!(!contract.locked, "contract should be unlocked");
            }
            _ => panic!("expected failure"),
        }
    }
}
