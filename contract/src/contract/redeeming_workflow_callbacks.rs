//required in order for near_bindgen macro to work outside of lib.rs
use crate::errors::illegal_state::STAKE_BATCH_SHOULD_EXIST;
use crate::*;
use crate::{
    domain::{self, RedeemLock},
    errors::{
        illegal_state::{
            ILLEGAL_REDEEM_LOCK_STATE, REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST,
            REDEEM_STAKE_BATCH_SHOULD_EXIST,
        },
        redeeming_stake_errors::UNSTAKED_FUNDS_NOT_AVAILABLE_FOR_WITHDRAWAL,
        staking_pool_failures::{
            GET_ACCOUNT_FAILURE, GET_STAKED_BALANCE_FAILURE, UNSTAKE_FAILURE, WITHDRAW_ALL_FAILURE,
        },
    },
    ext_redeeming_workflow_callbacks, ext_staking_pool,
    interface::BatchId,
    interface::Operator,
    near::{assert_predecessor_is_self, NO_DEPOSIT},
};
use near_sdk::{env, near_bindgen, Promise, PromiseOrValue};

#[near_bindgen]
impl StakeTokenContract {
    pub fn on_run_redeem_stake_batch(
        &mut self,
        #[callback] staked_balance: near_sdk::json_types::U128,
    ) -> Promise {
        assert_predecessor_is_self();
        // this callback should only be invoked when we are unstaking, i.e., when the RedeemStakeBatch
        // is kicked off
        assert!(self.is_unstaking(), ILLEGAL_REDEEM_LOCK_STATE);

        // the batch should always be present because the purpose of this callback is a step
        // in the batch processing workflow
        // - if the callback was called by itself, and the batch is not present, then there is a bug
        let batch = self
            .redeem_stake_batch
            .expect(REDEEM_STAKE_BATCH_SHOULD_EXIST);

        assert!(self.promise_result_succeeded(), GET_STAKED_BALANCE_FAILURE);

        // update the cached STAKE token value
        self.stake_token_value = self.stake_token_value(staked_balance.into());

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

        let unstaked_balance = staking_pool_account.unstaked_balance.0;
        // if unstaked balance is zero, then it means the unstaked NEAR funds were withdrawn, but the
        // workflow failed downstream, e.g., if not enough gas was supplied.
        //
        // When [run_redeem_stake_batch](crate::interface::StakingService::run_redeem_stake_batch] is
        // retried, then it can skip the fund withdrawal step
        if unstaked_balance > 0 {
            assert!(
                staking_pool_account.can_withdraw,
                UNSTAKED_FUNDS_NOT_AVAILABLE_FOR_WITHDRAWAL
            );

            return self
                .withdraw_all_funds_from_staking_pool()
                .then(self.invoke_on_redeeming_stake_post_withdrawal())
                .into();
        }

        PromiseOrValue::Value(self.finalize_redeem_batch())
    }

    pub fn on_redeeming_stake_post_withdrawal(&mut self) -> BatchId {
        assert_predecessor_is_self();
        assert!(self.promise_result_succeeded(), WITHDRAW_ALL_FAILURE);
        self.finalize_redeem_batch()
    }

    fn finalize_redeem_batch(&mut self) -> BatchId {
        let batch = self
            .redeem_stake_batch
            .expect(REDEEM_STAKE_BATCH_SHOULD_EXIST);
        let receipt = self
            .redeem_stake_batch_receipts
            .get(&batch.id())
            .expect(REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST);

        // update the total NEAR balance that is available for withdrawal
        self.total_near.credit(receipt.stake_near_value());

        self.run_redeem_stake_batch_lock = None;
        self.pop_redeem_stake_batch();

        batch.id().into()
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
        let batch = self.redeem_stake_batch.expect(STAKE_BATCH_SHOULD_EXIST);
        let batch_receipt = (batch, self.stake_token_value).into();
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

    pub(crate) fn invoke_on_redeeming_stake_post_withdrawal(&mut self) -> Promise {
        ext_redeeming_workflow_callbacks::on_redeeming_stake_post_withdrawal(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .callbacks()
                .on_redeeming_stake_post_withdrawal()
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
    use crate::interface::StakingService;
    use crate::{
        domain::{RedeemStakeBatch, TimestampedStakeBalance},
        near::YOCTO,
        test_utils::*,
    };
    use near_sdk::json_types::U128;
    use near_sdk::{serde::Deserialize, serde_json, testing_env, MockedBlockchain};

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct UnstakeArgs {
        amount: String,
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
        let mut contract = StakeTokenContract::new(None, contract_settings);
        *contract.batch_id_sequence += 1;
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        let redeem_stake_batch =
            RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        let staked_balance: U128 = (1100 * YOCTO).into();
        contract.on_run_redeem_stake_batch(staked_balance.clone());
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
                        .stake_token_value(staked_balance.0.into())
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
        let mut contract = StakeTokenContract::new(None, contract_settings);
        *contract.batch_id_sequence += 1;
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        let redeem_stake_batch =
            RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        let staked_balance: U128 = (1100 * YOCTO).into();
        contract.on_run_redeem_stake_batch(staked_balance);
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

                    let args: UnstakeArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(args.amount, (110 * YOCTO).to_string());
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
            {
                let receipt = &receipts[1];
                assert_eq!(receipt.receiver_id, context.current_account_id);
                match &receipt.actions[0] {
                    Action::FunctionCall {
                        method_name, gas, ..
                    } => {
                        assert_eq!(method_name, "on_unstake");

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
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed internally")]
    fn on_run_redeem_stake_batch_invoked_by_non_self() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.on_run_redeem_stake_batch((YOCTO).into());
    }

    #[test]
    #[should_panic(expected = "ILLEGAL STATE : redeem stake batch should exist")]
    fn on_run_redeem_stake_batch_invoked_illegal_state_no_redeem_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);
        contract.on_run_redeem_stake_batch(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed internally")]
    fn on_unstake_invoked_by_non_self() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

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
        let mut contract = StakeTokenContract::new(None, contract_settings);
        *contract.batch_id_sequence += 1;

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);
        let redeem_stake_batch =
            RedeemStakeBatch::new(contract.batch_id_sequence, (100 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.total_stake = TimestampedStakeBalance::new((1000 * YOCTO).into());
        let staked_near_balance = (1100 * YOCTO).into();
        contract.stake_token_value = contract.stake_token_value(staked_near_balance);

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
        assert_eq!(
            receipt.stake_token_value().total_stake_supply(),
            contract.total_stake.amount() + receipt.redeemed_stake()
        );
        assert_eq!(
            receipt.stake_token_value().total_staked_near_balance(),
            staked_near_balance
        );

        let receipt = contract.pending_withdrawal().unwrap();
        assert_eq!(receipt.redeemed_stake, (100 * YOCTO).into());
        assert_eq!(
            contract.run_redeem_stake_batch_lock,
            Some(RedeemLock::PendingWithdrawal)
        );
    }

    #[test]
    #[should_panic(expected = "failed to unstake NEAR with staking pool")]
    fn on_unstake_staking_pool_failure() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
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
        let mut contract = StakeTokenContract::new(None, contract_settings);
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
        let mut contract = StakeTokenContract::new(None, contract_settings);
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
        assert_eq!(receipts.len(), 2);
        {
            let receipt = &receipts[0];
            assert_eq!(receipt.receiver_id, contract.staking_pool_id);
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name,
                    gas,
                    args,
                    ..
                } => {
                    assert_eq!(method_name, "withdraw_all");
                    assert!(args.is_empty());
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
            assert_eq!(receipt.receiver_id, env::current_account_id());
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name,
                    args,
                    gas,
                    ..
                } => {
                    assert_eq!(method_name, "on_redeeming_stake_post_withdrawal");
                    assert!(args.is_empty());
                    assert_eq!(
                        contract
                            .config
                            .gas_config()
                            .callbacks()
                            .on_redeeming_stake_post_withdrawal()
                            .value(),
                        *gas
                    );
                }
                _ => panic!("expected FunctionCall"),
            }
        }
    }

    #[test]
    fn serialize_u128() {
        let value = U128(2832187358794090528436378);
        let json_value = serde_json::to_string(&value).unwrap();
        println!("{}", json_value);
    }
}
