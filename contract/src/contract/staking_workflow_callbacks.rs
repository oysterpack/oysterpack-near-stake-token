//required in order for near_bindgen macro to work outside of lib.rs
use crate::*;
use crate::{
    domain::{self, YoctoNear},
    errors::{
        illegal_state::{REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST, STAKE_BATCH_SHOULD_EXIST},
        staking_pool_failures::{DEPOSIT_AND_STAKE_FAILURE, GET_STAKED_BALANCE_FAILURE},
    },
    ext_staking_pool, ext_staking_workflow_callbacks,
    near::{assert_predecessor_is_self, NO_DEPOSIT},
};
use near_sdk::{env, near_bindgen, Promise};

#[near_bindgen]
impl StakeTokenContract {
    /// part of the [run_stake_batch] workflow
    pub fn on_run_stake_batch(
        &mut self,
        #[callback] staking_pool_account: StakingPoolAccount,
    ) -> Promise {
        assert_predecessor_is_self();

        // the batch should always be present because the purpose of this callback is a step
        // in the batch processing workflow
        // - if the callback was called by itself, and the batch is not present, then there is a bug
        let batch = self.stake_batch.expect(STAKE_BATCH_SHOULD_EXIST);

        assert!(self.promise_result_succeeded(), GET_STAKED_BALANCE_FAILURE);

        // update the cached STAKE token value
        let staked_balance = staking_pool_account.staked_balance;
        self.stake_token_value = self.stake_token_value(staked_balance.into());

        let deposit_and_stake = || {
            self.invoke_deposit_and_stake(batch.balance().amount())
                .then(self.invoke_on_deposit_and_stake())
        };

        let unstaked_balance = staking_pool_account.unstaked_balance.0;
        match self.run_redeem_stake_batch_lock {
            Some(RedeemLock::PendingWithdrawal) if unstaked_balance > 0 => {
                let pending_receipt = self
                    .get_pending_withdrawal()
                    .expect(REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST);
                if self.near_liquidity_pool < pending_receipt.stake_near_value() {
                    self.add_liquidity_then_deposit_and_stake(unstaked_balance, batch)
                } else {
                    deposit_and_stake()
                }
            }
            _ => deposit_and_stake(),
        }
    }

    /// ## Success Workflow
    /// 1. create [StakeBatchReceipt]
    /// 2. update total STAKE supply
    /// 3. clear the current STAKE batch
    /// 4. move the next batch into the current batch
    pub fn on_deposit_and_stake(&mut self) {
        assert_predecessor_is_self();

        let batch = self.stake_batch.take().expect(STAKE_BATCH_SHOULD_EXIST);
        assert!(
            self.all_promise_results_succeeded(),
            DEPOSIT_AND_STAKE_FAILURE
        );

        self.create_stake_batch_receipt(batch);
        self.pop_stake_batch();
    }
}

impl StakeTokenContract {
    fn add_liquidity_then_deposit_and_stake(
        &mut self,
        unstaked_balance: u128,
        batch: StakeBatch,
    ) -> Promise {
        // compute how much NEAR liquidity can be transferred from the unstaked NEAR to the liquidity pool
        let near_liquidity = if unstaked_balance >= batch.balance().amount().value() {
            batch.balance().amount().value()
        } else {
            unstaked_balance
        };
        *self.near_liquidity_pool += near_liquidity;
        let deposit_amount = batch.balance().amount().value() - near_liquidity;
        if deposit_amount > 0 {
            self.invoke_deposit(deposit_amount.into())
                .then(self.invoke_stake(batch.balance().amount()))
                .then(self.invoke_on_deposit_and_stake())
        } else {
            self.invoke_stake(batch.balance().amount())
                .then(self.invoke_on_deposit_and_stake())
        }
    }

    fn create_stake_batch_receipt(&mut self, batch: domain::StakeBatch) {
        let stake_batch_receipt =
            domain::StakeBatchReceipt::new(batch.balance().amount(), self.stake_token_value);
        self.stake_batch_receipts
            .insert(&batch.id(), &stake_batch_receipt);

        // update the total STAKE supply
        let stake_amount = self
            .stake_token_value
            .near_to_stake(batch.balance().amount());
        self.total_stake.credit(stake_amount);
    }

    /// moves the next batch into the current batch
    fn pop_stake_batch(&mut self) {
        self.stake_batch = self.next_stake_batch.take();
    }
}

/// staking NEAR workflow callback invocations
impl StakeTokenContract {
    fn invoke_deposit(&self, amount: YoctoNear) -> Promise {
        ext_staking_pool::deposit(
            &self.staking_pool_id,
            amount.value(),
            self.config
                .gas_config()
                .staking_pool()
                .deposit_and_stake()
                .value(),
        )
    }

    fn invoke_stake(&self, amount: YoctoNear) -> Promise {
        ext_staking_pool::stake(
            amount.value().into(),
            &self.staking_pool_id,
            NO_DEPOSIT.value(),
            self.config
                .gas_config()
                .staking_pool()
                .deposit_and_stake()
                .value(),
        )
    }

    fn invoke_deposit_and_stake(&self, amount: YoctoNear) -> Promise {
        ext_staking_pool::deposit_and_stake(
            &self.staking_pool_id,
            amount.value(),
            self.config
                .gas_config()
                .staking_pool()
                .deposit_and_stake()
                .value(),
        )
    }

    pub(crate) fn invoke_on_run_stake_batch(&self) -> Promise {
        ext_staking_workflow_callbacks::on_run_stake_batch(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .callbacks()
                .on_run_stake_batch()
                .value(),
        )
    }

    pub(crate) fn invoke_release_run_stake_batch_lock(&self) -> Promise {
        ext_staking_workflow_callbacks::release_run_stake_batch_lock(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config.gas_config().callbacks().unlock().value(),
        )
    }

    pub(crate) fn invoke_on_deposit_and_stake(&self) -> Promise {
        ext_staking_workflow_callbacks::on_deposit_and_stake(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .callbacks()
                .on_deposit_and_stake()
                .value(),
        )
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::{
        interface::{AccountManagement, StakingService},
        near::YOCTO,
        test_utils::*,
    };
    use near_sdk::{serde_json, testing_env, MockedBlockchain};

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
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        let initial_stake_token_value = contract.stake_token_value;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        // account deposits into stake batch
        contract.deposit();
        contract.stake();

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        context.attached_deposit = 100 * YOCTO;
        context.epoch_height += 1;
        testing_env!(context.clone());
        let staking_pool_account = StakingPoolAccount {
            account_id: context.predecessor_account_id.clone(),
            unstaked_balance: 0.into(),
            staked_balance: 0.into(),
            can_withdraw: true,
        };
        contract.on_run_stake_batch(staking_pool_account);
        let stake_token_value_after_callback = contract.stake_token_value;
        assert!(
            stake_token_value_after_callback
                .block_time_height()
                .epoch_height()
                .value()
                > initial_stake_token_value
                    .block_time_height()
                    .epoch_height()
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

    /// Given there is a pending withdrawal
    /// And the amount of unstaked NEAR is more than is being staked
    /// When the callback is invoked
    /// Then the entire stake batch NEAR amount is added to the liquidity pool
    /// And a stake request is submitted to the staking pool
    #[test]
    fn on_run_stake_batch_success_with_pending_withdrawal_with_all_near_added_to_liquidity() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        let initial_stake_token_value = contract.stake_token_value;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        // account deposits into stake batch
        contract.deposit();
        contract.stake();

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        context.attached_deposit = 100 * YOCTO;
        context.epoch_height += 1;
        testing_env!(context.clone());
        let staking_pool_account = StakingPoolAccount {
            account_id: context.predecessor_account_id.clone(),
            unstaked_balance: (200 * YOCTO).into(),
            staked_balance: 0.into(),
            can_withdraw: true,
        };

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        *contract.batch_id_sequence += 1;
        let redeem_stake_batch =
            domain::RedeemStakeBatch::new(contract.batch_id_sequence, (10 * YOCTO).into());
        let receipt =
            domain::RedeemStakeBatchReceipt::from((redeem_stake_batch, contract.stake_token_value));
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract
            .redeem_stake_batch_receipts
            .insert(&redeem_stake_batch.id(), &receipt);
        contract.on_run_stake_batch(staking_pool_account);
        assert_eq!(
            contract.near_liquidity_pool,
            contract.stake_batch.unwrap().balance().amount()
        );
        let stake_token_value_after_callback = contract.stake_token_value;
        assert!(
            stake_token_value_after_callback
                .block_time_height()
                .epoch_height()
                .value()
                > initial_stake_token_value
                    .block_time_height()
                    .epoch_height()
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
                            method_name == "stake"
                                && *deposit == 0
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

    /// Given there is a pending withdrawal
    /// And the amount of unstaked NEAR is less than what is being staked
    /// When the callback is invoked
    /// Then the entire partial batch NEAR amount is added to the liquidity pool
    /// And a deposit request and then a stake request are submitted to the staking pool
    #[test]
    fn on_run_stake_batch_success_with_pending_withdrawal_with_partial_near_added_to_liquidity() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        let initial_stake_token_value = contract.stake_token_value;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        // account deposits into stake batch
        contract.deposit();
        contract.stake();

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        context.attached_deposit = 100 * YOCTO;
        context.epoch_height += 1;
        testing_env!(context.clone());
        let staking_pool_account = StakingPoolAccount {
            account_id: context.predecessor_account_id.clone(),
            unstaked_balance: (40 * YOCTO).into(),
            staked_balance: 0.into(),
            can_withdraw: true,
        };

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        *contract.batch_id_sequence += 1;
        let redeem_stake_batch =
            domain::RedeemStakeBatch::new(contract.batch_id_sequence, (10 * YOCTO).into());
        let receipt =
            domain::RedeemStakeBatchReceipt::from((redeem_stake_batch, contract.stake_token_value));
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract
            .redeem_stake_batch_receipts
            .insert(&redeem_stake_batch.id(), &receipt);
        contract.on_run_stake_batch(staking_pool_account.clone());
        assert_eq!(
            contract.near_liquidity_pool,
            staking_pool_account.unstaked_balance.into()
        );
        let stake_token_value_after_callback = contract.stake_token_value;
        assert!(
            stake_token_value_after_callback
                .block_time_height()
                .epoch_height()
                .value()
                > initial_stake_token_value
                    .block_time_height()
                    .epoch_height()
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
        assert_eq!(receipts.len(), 3);

        // check `deposit_and_stake` func call action
        {
            let receipt = &receipts[0];
            let action = &receipt.actions[0];
            if let Action::FunctionCall {
                method_name,
                deposit,
                gas,
                ..
            } = action
            {
                assert_eq!(method_name, "deposit");
                assert_eq!(
                    *deposit,
                    contract.stake_batch.unwrap().balance().amount().value()
                        - staking_pool_account.unstaked_balance.0
                );
                assert_eq!(
                    *gas,
                    contract
                        .config
                        .gas_config()
                        .staking_pool()
                        .deposit_and_stake()
                        .value()
                )
            } else {
                panic!("expected deposit function call")
            }
        }
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
                            method_name == "stake"
                                && *deposit == 0
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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();
        contract.stake();

        assert!(contract.run_stake_batch_locked);

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());
        set_env_with_failed_promise_result(&mut contract);
        let staking_pool_account = StakingPoolAccount {
            account_id: context.predecessor_account_id,
            unstaked_balance: 0.into(),
            staked_balance: 0.into(),
            can_withdraw: true,
        };
        contract.on_run_stake_batch(staking_pool_account);
    }
}
