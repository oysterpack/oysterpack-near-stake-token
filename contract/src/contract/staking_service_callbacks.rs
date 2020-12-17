use crate::domain::RedeemLock;
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
    pub fn on_run_stake_batch(&mut self, #[callback] staked_balance: Balance) -> Promise {
        assert_predecessor_is_self();

        // the batch should always be present because the purpose of this callback is a step
        // in the batch processing workflow
        // - if the callback was called by itself, and the batch is not present, then there is a bug
        let batch = self.stake_batch.expect("stake batch must be present");

        assert!(
            self.promise_result_succeeded(),
            "failed to get staked balance from staking pool"
        );

        // update the cached STAKE token value
        self.stake_token_value =
            domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.balance());

        let deposit_and_stake = ext_staking_pool::deposit_and_stake(
            &self.staking_pool_id,
            batch.balance().balance().value(),
            self.config
                .gas_config()
                .staking_pool()
                .deposit_and_stake()
                .value(),
        );

        let on_deposit_and_stake = ext_staking_pool_callbacks::on_deposit_and_stake(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .callbacks()
                .on_deposit_and_stake()
                .value(),
        );

        deposit_and_stake.then(on_deposit_and_stake)
    }

    /// ## Success Workflow
    /// 1. create [StakeBatchReceipt]
    /// 2. update total STAKE supply
    /// 3. clear the current STAKE batch
    /// 4. move the next batch into the current batch
    pub fn on_deposit_and_stake(&mut self) {
        assert_predecessor_is_self();

        let batch = self
            .stake_batch
            .take()
            .expect("callback should only be invoked when there is a StakeBatch being processed");
        assert!(self.promise_result_succeeded(),"ERR: failed to process stake batch #{} - `deposit_and_stake` func call on staking pool failed", batch.id().value());

        // create batch receipt
        let stake_batch_receipt =
            domain::StakeBatchReceipt::new(batch.balance().balance(), self.stake_token_value);
        self.stake_batch_receipts
            .insert(&batch.id(), &stake_batch_receipt);

        // update the total STAKE supply
        self.total_stake.credit(
            self.stake_token_value
                .near_to_stake(batch.balance().balance()),
        );

        // move the next batch into the current batch
        self.stake_batch = self.next_stake_batch.take();
    }

    pub fn on_run_redeem_stake_batch(&mut self, #[callback] staked_balance: Balance) -> Promise {
        assert_predecessor_is_self();

        // the batch should always be present because the purpose of this callback is a step
        // in the batch processing workflow
        // - if the callback was called by itself, and the batch is not present, then there is a bug
        let batch = self
            .redeem_stake_batch
            .expect("redeem stake batch must be present");

        assert!(
            self.promise_result_succeeded(),
            "failed to get staked balance from staking pool"
        );

        // update the cached STAKE token value
        self.stake_token_value =
            domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.balance());

        let unstake_amount = self
            .stake_token_value
            .stake_to_near(batch.balance().balance());

        let unstake = ext_staking_pool::unstake(
            unstake_amount.value().into(),
            &self.staking_pool_id,
            NO_DEPOSIT.value(),
            self.config.gas_config().staking_pool().unstake().value(),
        );

        let on_unstake = ext_staking_pool_callbacks::on_unstake(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config.gas_config().callbacks().on_unstake().value(),
        );

        unstake.then(on_unstake)
    }

    /// ## Success Workflow
    pub fn on_unstake(&mut self) {
        assert_predecessor_is_self();

        let batch = self.redeem_stake_batch.take().expect(
            "callback should only be invoked when there is a RedeemStakeBatch being processed",
        );

        assert!(self.promise_result_succeeded(),"ERR: failed to process stake batch #{} - `on_unstake` func call on staking pool failed", batch.id().value());

        // create batch receipt
        let batch_receipt =
            domain::RedeemStakeBatchReceipt::new(batch.balance().balance(), self.stake_token_value);
        self.redeem_stake_batch_receipts
            .insert(&batch.id(), &batch_receipt);

        // update the total STAKE supply
        self.total_stake.debit(batch.balance().balance());

        // move the next batch into the current batch
        self.redeem_stake_batch = self.next_redeem_stake_batch.take();
        // progress the workflow to pending withdrawal
        self.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal(batch.id()))
    }

    /// ## Workflow
    /// 1. clear the [run_redeem_stake_batch] lock
    /// 2. try running the redeem stake batch  
    ///
    /// ## Panics
    /// - not invoked by self
    /// - if withdrawal from staking pool failed
    pub fn on_staking_pool_withdrawal(&mut self, redeem_stake_batch_id: BatchId) -> Promise {
        assert_predecessor_is_self();

        assert!(
            self.promise_result_succeeded(),
            "failed to withdraw unstaked balance from staking pool"
        );

        self.run_redeem_stake_batch_lock = None;
        self.run_redeem_stake_batch()
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::interface::Operator;
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

                        let receipt = contract.stake_batch_receipts.get(&batch_id).expect(
                            "receipt should have been created by `on_deposit_and_stake` callback",
                        );

                        assert_eq!(
                            contract.total_stake.balance(),
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
