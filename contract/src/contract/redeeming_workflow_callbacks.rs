//required in order for near_bindgen macro to work outside of lib.rs
use crate::*;
use crate::{
    domain::{self, RedeemLock},
    errors::{
        illegal_state::{REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST, REDEEM_STAKE_BATCH_SHOULD_EXIST},
        redeeming_stake_errors::{
            UNSTAKED_FUNDS_NOT_AVAILABLE_FOR_WITHDRAWAL, UNSTAKING_BLOCKED_BY_PENDING_WITHDRAWAL,
        },
        staking_pool_failures::{GET_ACCOUNT_FAILURE, GET_STAKED_BALANCE_FAILURE, UNSTAKE_FAILURE},
    },
    ext_redeeming_workflow_callbacks, ext_staking_pool,
    interface::{BatchId, Operator},
    near::{assert_predecessor_is_self, NO_DEPOSIT},
};
use near_sdk::{env, near_bindgen, Promise, PromiseOrValue};

#[near_bindgen]
impl StakeTokenContract {
    pub fn on_run_redeem_stake_batch(
        &mut self,
        #[callback] staking_pool_account: StakingPoolAccount,
    ) -> Promise {
        assert_predecessor_is_self();

        // the batch should always be present because the purpose of this callback is a step
        // in the batch processing workflow
        // - if the callback was called by itself, and the batch is not present, then there is a bug
        let batch = self
            .redeem_stake_batch
            .expect(REDEEM_STAKE_BATCH_SHOULD_EXIST);

        assert!(self.promise_result_succeeded(), GET_STAKED_BALANCE_FAILURE);

        // all unstaked NEAR must be withdrawn before we are allowed to unstake more NEAR
        // - per the staking pool contract, unstaked NEAR funds are locked for 4 epoch periods and
        //   if more funds are unstaked, then the lock period resets to 4 epochs
        if staking_pool_account.unstaked_balance.0 > 0 {
            assert!(
                staking_pool_account.can_withdraw,
                UNSTAKING_BLOCKED_BY_PENDING_WITHDRAWAL
            );

            return self
                .withdraw_all_funds_from_staking_pool()
                .then(self.get_account_from_staking_pool())
                .then(self.invoke_on_run_redeem_stake_batch());
        }

        // update the cached STAKE token value
        self.stake_token_value = domain::StakeTokenValue::new(
            staking_pool_account.staked_balance.0.into(),
            self.total_stake.amount(),
        );

        let unstake_amount = self
            .stake_token_value
            .stake_to_near(batch.balance().amount());

        self.unstake(unstake_amount).then(self.invoke_on_unstake())
    }

    pub fn on_unstake(&mut self) {
        assert_predecessor_is_self();

        assert!(self.promise_result_succeeded(), UNSTAKE_FAILURE);

        self.create_redeem_stake_batch_receipt();
        self.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal)
    }

    pub fn on_redeeming_stake_pending_withdrawal(
        &mut self,
        #[callback] staking_pool_account: StakingPoolAccount,
    ) -> PromiseOrValue<BatchId> {
        assert_predecessor_is_self();

        assert!(self.promise_result_succeeded(), GET_ACCOUNT_FAILURE);

        if staking_pool_account.unstaked_balance.0 > 0 {
            assert!(
                staking_pool_account.can_withdraw,
                UNSTAKED_FUNDS_NOT_AVAILABLE_FOR_WITHDRAWAL
            );

            return self
                .withdraw_all_funds_from_staking_pool()
                .then(self.get_account_from_staking_pool())
                .then(self.invoke_on_redeeming_stake_pending_withdrawal())
                .into();
        }

        let batch_id = self
            .redeem_stake_batch
            .expect(REDEEM_STAKE_BATCH_SHOULD_EXIST)
            .id();

        let receipt = self
            .redeem_stake_batch_receipts
            .get(&batch_id)
            .expect(REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST);
        self.total_near.credit(receipt.stake_near_value());

        self.run_redeem_stake_batch_lock = None;
        self.pop_redeem_stake_batch();
        PromiseOrValue::Value(batch_id.into())
    }
}

impl StakeTokenContract {
    fn unstake(&self, unstake_amount: domain::YoctoNear) -> Promise {
        ext_staking_pool::unstake(
            unstake_amount.value().into(),
            &self.staking_pool_id,
            NO_DEPOSIT.value(),
            self.config.gas_config().staking_pool().unstake().value(),
        )
    }

    fn create_redeem_stake_batch_receipt(&mut self) {
        let batch = self
            .redeem_stake_batch
            .expect("illegal state - batch should exist");

        // create batch receipt
        let batch_receipt =
            domain::RedeemStakeBatchReceipt::new(batch.balance().amount(), self.stake_token_value);
        self.redeem_stake_batch_receipts
            .insert(&batch.id(), &batch_receipt);

        // update the total STAKE supply
        self.total_stake.debit(batch_receipt.redeemed_stake());
    }

    /// moves the next batch into the current batch
    fn pop_redeem_stake_batch(&mut self) {
        self.redeem_stake_batch = self.next_redeem_stake_batch.take();
    }
}

/// redeeming STAKE workflow callback invocations
impl StakeTokenContract {
    pub(crate) fn invoke_on_run_redeem_stake_batch(&self) -> Promise {
        ext_redeeming_workflow_callbacks::on_run_redeem_stake_batch(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .callbacks()
                .on_run_redeem_stake_batch()
                .value(),
        )
    }

    pub(crate) fn invoke_release_run_redeem_stake_batch_unstaking_lock(&self) -> Promise {
        ext_redeeming_workflow_callbacks::release_run_redeem_stake_batch_unstaking_lock(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config.gas_config().callbacks().unlock().value(),
        )
    }

    pub(crate) fn invoke_on_redeeming_stake_pending_withdrawal(&mut self) -> Promise {
        ext_redeeming_workflow_callbacks::on_redeeming_stake_pending_withdrawal(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .callbacks()
                .on_redeeming_stake_pending_withdrawal()
                .value(),
        )
    }

    pub(crate) fn invoke_on_unstake(&self) -> Promise {
        ext_redeeming_workflow_callbacks::on_unstake(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config.gas_config().callbacks().on_unstake().value(),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::domain::RedeemStakeBatchReceipt;
    use crate::{
        domain::{RedeemStakeBatch, TimestampedStakeBalance},
        near::YOCTO,
        test_utils::*,
    };
    use near_sdk::{serde::Deserialize, serde_json, testing_env, MockedBlockchain};

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct GetAccountArgs {
        account_id: String,
    }

    /// When there are no unstaked NEAR funds in the staking pool
    /// Then update the STAKE token value
    /// And submit an unstake request to the staking pool
    #[test]
    fn on_run_redeem_stake_batch_with_zero_unstaked_balance() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        *contract.batch_id_sequence += 1;

        let redeem_stake_batch =
            RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        let staking_pool_account = StakingPoolAccount {
            account_id: context.current_account_id.to_string(),
            unstaked_balance: 0.into(),
            staked_balance: (1100 * YOCTO).into(),
            can_withdraw: true,
        };
        contract.on_run_redeem_stake_batch(staking_pool_account);
        let receipts = deserialize_receipts(&env::created_receipts());
        println!("{:#?}", receipts);
        assert_eq!(receipts.len(), 2);
        {
            let receipt = &receipts[0];
            assert_eq!(receipt.receiver_id, contract.staking_pool_id);
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name,
                    args,
                    gas,
                    ..
                } => {
                    assert_eq!(method_name, "unstake");

                    let unstake_amount = contract
                        .stake_token_value
                        .stake_to_near(contract.redeem_stake_batch.unwrap().balance().amount());
                    assert!(args.contains(&unstake_amount.value().to_string()));
                    assert_eq!(
                        contract
                            .config
                            .gas_config()
                            .staking_pool()
                            .unstake()
                            .value(),
                        *gas
                    );
                }
                _ => panic!("expected FunctionCall"),
            }
        }
        {
            let receipt = &receipts[1];
            assert_eq!(receipt.receiver_id, context.current_account_id);
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name,
                    args,
                    gas,
                    ..
                } => {
                    assert_eq!(method_name, "on_unstake");

                    let unstake_amount = contract
                        .stake_token_value
                        .stake_to_near(contract.redeem_stake_batch.unwrap().balance().amount());
                    assert!(args.is_empty());
                    assert_eq!(
                        contract
                            .config
                            .gas_config()
                            .callbacks()
                            .on_unstake()
                            .value(),
                        *gas
                    );
                }
                _ => panic!("expected FunctionCall"),
            }
        }
    }

    /// When there are unstaked NEAR funds in the staking pool
    /// And the unstaked funds can be withdrawn
    /// Then a request to withdraw all funds is sent to the staking pool
    /// And then the account is retrieved from the staking pool
    /// And then the redeem batch is retried
    #[test]
    fn on_run_redeem_stake_batch_with_nonzero_unstaked_balance_and_can_withdraw() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        *contract.batch_id_sequence += 1;

        let redeem_stake_batch =
            RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        let staking_pool_account = StakingPoolAccount {
            account_id: context.current_account_id.to_string(),
            unstaked_balance: (100 * YOCTO).into(),
            staked_balance: (1100 * YOCTO).into(),
            can_withdraw: true,
        };
        contract.on_run_redeem_stake_batch(staking_pool_account);
        let receipts = deserialize_receipts(&env::created_receipts());
        println!("{:#?}", receipts);
        assert_eq!(receipts.len(), 3);
        {
            let receipt = &receipts[0];
            assert_eq!(receipt.receiver_id, contract.staking_pool_id);
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name, gas, ..
                } => {
                    assert_eq!(method_name, "withdraw_all");
                    assert_eq!(
                        contract
                            .config
                            .gas_config()
                            .staking_pool()
                            .withdraw()
                            .value(),
                        *gas
                    );
                }
                _ => panic!("expected FunctionCall"),
            }
        }
        {
            let receipt = &receipts[1];
            assert_eq!(receipt.receiver_id, contract.staking_pool_id);
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name,
                    args,
                    gas,
                    ..
                } => {
                    assert_eq!(method_name, "get_account");

                    let args: GetAccountArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(args.account_id, context.current_account_id);
                    assert_eq!(
                        contract
                            .config
                            .gas_config()
                            .callbacks()
                            .on_unstake()
                            .value(),
                        *gas
                    );
                }
                _ => panic!("expected FunctionCall"),
            }
            {
                let receipt = &receipts[2];
                assert_eq!(receipt.receiver_id, context.current_account_id);
                match &receipt.actions[0] {
                    Action::FunctionCall {
                        method_name, gas, ..
                    } => {
                        assert_eq!(method_name, "on_run_redeem_stake_batch");

                        assert_eq!(
                            contract
                                .config
                                .gas_config()
                                .callbacks()
                                .on_run_redeem_stake_batch()
                                .value(),
                            *gas
                        );
                    }
                    _ => panic!("expected FunctionCall"),
                }
            }
        }
    }

    // When there are unstaked NEAR funds in the staking pool
    /// And the unstaked funds are not available for withdrawal
    /// Then the txn fails
    #[test]
    #[should_panic(expected = "unstaking is blocked until all unstaked NEAR can be withdrawn")]
    fn on_run_redeem_stake_batch_with_nonzero_unstaked_balance_and_cannot_withdraw() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        *contract.batch_id_sequence += 1;

        let redeem_stake_batch =
            RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        let staking_pool_account = StakingPoolAccount {
            account_id: context.current_account_id.to_string(),
            unstaked_balance: (100 * YOCTO).into(),
            staked_balance: (1100 * YOCTO).into(),
            can_withdraw: false,
        };
        contract.on_run_redeem_stake_batch(staking_pool_account);
    }

    #[test]
    #[should_panic(expected = "func call is only allowed internally")]
    fn on_run_redeem_stake_batch_invoked_by_non_self() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        let staking_pool_account = StakingPoolAccount {
            account_id: context.current_account_id.to_string(),
            unstaked_balance: (100 * YOCTO).into(),
            staked_balance: (1100 * YOCTO).into(),
            can_withdraw: false,
        };
        contract.on_run_redeem_stake_batch(staking_pool_account);
    }

    #[test]
    #[should_panic(expected = "ILLEGAL STATE : redeem stake batch should exist")]
    fn on_run_redeem_stake_batch_invoked_illegal_state_no_redeem_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        let staking_pool_account = StakingPoolAccount {
            account_id: context.current_account_id.to_string(),
            unstaked_balance: (100 * YOCTO).into(),
            staked_balance: (1100 * YOCTO).into(),
            can_withdraw: false,
        };
        contract.on_run_redeem_stake_batch(staking_pool_account);
    }

    #[test]
    #[should_panic(expected = "func call is only allowed internally")]
    fn on_unstake_invoked_by_non_self() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.on_unstake();
    }

    /// When on_unstake is invoked
    /// Then batch receipt is created
    /// And the total STAKE supply is reduced
    #[test]
    fn on_unstake_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        *contract.batch_id_sequence += 1;
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        let redeem_stake_batch =
            RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        context.predecessor_account_id = context.current_account_id.clone();
        context.epoch_height += 1;
        context.block_index += 1;
        context.block_timestamp += 1;
        testing_env!(context.clone());
        contract.on_unstake();

        assert_eq!(contract.total_stake.amount(), (900 * YOCTO).into());
        let receipt = contract
            .redeem_stake_batch_receipts
            .get(&contract.redeem_stake_batch.unwrap().id())
            .unwrap();
        assert_eq!(receipt.redeemed_stake(), (100 * YOCTO).into());
    }

    #[test]
    #[should_panic(expected = "failed to unstake NEAR with staking pool")]
    fn on_unstake_staking_pool_failure() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        *contract.batch_id_sequence += 1;
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        let redeem_stake_batch =
            RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());
        set_env_with_failed_promise_result(&mut contract);
        contract.on_unstake();
    }

    /// Given the unstaked balance with the staking pool is 0
    /// Then the redeem lock is set to None
    /// And the redeem stake batch is popped
    #[test]
    fn on_redeeming_stake_pending_withdrawal_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        *contract.batch_id_sequence += 1;
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        let batch = RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        let batch_receipt =
            RedeemStakeBatchReceipt::new(batch.balance().amount(), contract.stake_token_value);
        contract
            .redeem_stake_batch_receipts
            .insert(&batch.id(), &batch_receipt);
        let stake_near_value = batch_receipt.stake_near_value();

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        let staking_pool_account = StakingPoolAccount {
            account_id: context.current_account_id.to_string(),
            unstaked_balance: 0.into(),
            staked_balance: (1100 * YOCTO).into(),
            can_withdraw: true,
        };
        match contract.on_redeeming_stake_pending_withdrawal(staking_pool_account) {
            PromiseOrValue::Value(batch_id) => assert_eq!(batch_id, batch.id().into()),
            _ => panic!("redeem stake batch should have completed"),
        }
        assert!(contract.redeem_stake_batch.is_none());
        println!("contract NEAR balance = {:?}", contract.total_near.amount());
        assert_eq!(contract.total_near.amount(), stake_near_value);
    }

    /// Given the unstaked balance with the staking pool is > 0
    /// And the unstaked funds can be withdrawn
    /// Then the all funds are withdrawn from the staking pool
    /// And the account is retrieved from the staking pool
    /// And the callback is retried
    #[test]
    fn on_redeeming_stake_pending_withdrawal_with_unstaked_funds_can_withdraw() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        *contract.batch_id_sequence += 1;
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        let redeem_stake_batch =
            RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        let staking_pool_account = StakingPoolAccount {
            account_id: context.current_account_id.to_string(),
            unstaked_balance: 1000.into(),
            staked_balance: (1100 * YOCTO).into(),
            can_withdraw: true,
        };
        contract.on_redeeming_stake_pending_withdrawal(staking_pool_account);
        let receipts = deserialize_receipts(&env::created_receipts());
        println!("{:#?}", receipts);
        assert_eq!(receipts.len(), 3);
        {
            let receipt = &receipts[0];
            assert_eq!(receipt.receiver_id, contract.staking_pool_id);
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name, gas, ..
                } => {
                    assert_eq!(method_name, "withdraw_all");
                    assert_eq!(
                        contract
                            .config
                            .gas_config()
                            .staking_pool()
                            .withdraw()
                            .value(),
                        *gas
                    );
                }
                _ => panic!("expected FunctionCall"),
            }
        }
        {
            let receipt = &receipts[1];
            assert_eq!(receipt.receiver_id, contract.staking_pool_id);
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name,
                    args,
                    gas,
                    ..
                } => {
                    assert_eq!(method_name, "get_account");

                    let args: GetAccountArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(args.account_id, context.current_account_id);
                    assert_eq!(
                        contract
                            .config
                            .gas_config()
                            .callbacks()
                            .on_unstake()
                            .value(),
                        *gas
                    );
                }
                _ => panic!("expected FunctionCall"),
            }
            {
                let receipt = &receipts[2];
                assert_eq!(receipt.receiver_id, context.current_account_id);
                match &receipt.actions[0] {
                    Action::FunctionCall {
                        method_name, gas, ..
                    } => {
                        assert_eq!(method_name, "on_redeeming_stake_pending_withdrawal");

                        assert_eq!(
                            contract
                                .config
                                .gas_config()
                                .callbacks()
                                .on_run_redeem_stake_batch()
                                .value(),
                            *gas
                        );
                    }
                    _ => panic!("expected FunctionCall"),
                }
            }
        }
    }
}
