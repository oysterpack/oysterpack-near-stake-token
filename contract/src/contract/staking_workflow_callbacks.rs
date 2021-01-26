//required in order for near_bindgen macro to work outside of lib.rs
use crate::*;
use crate::{
    domain::{self, YoctoNear, YoctoStake, TGAS},
    errors::illegal_state::STAKE_BATCH_SHOULD_EXIST,
    ext_staking_workflow_callbacks,
    interface::staking_service::events::{NearLiquidityAdded, PendingWithdrawalCleared, Staked},
    near::{log, NO_DEPOSIT},
};
use near_sdk::{env, near_bindgen, Promise};

#[near_bindgen]
impl StakeTokenContract {
    /// if unstaked balance is non-zero and liquidity is needed for pending withdrawal, then
    /// [add_liquidity_then_deposit_and_stake](StakeTokenContract::add_liquidity_then_deposit_and_stake)
    ///
    /// else kickoff the following promise chain:
    /// 1. deposit and stake funds into staking pool
    /// 2. get account from staking pool
    /// 3. invoke `on_deposit_and_stake` callback
    ///
    /// ## Panics
    /// - if not called by self
    /// - if there is no [StakeBatch](crate::domain::StakeBatch)
    /// - if the upstream promise to get the account from the staking pool failed
    #[private]
    pub fn on_run_stake_batch(
        &mut self,
        #[callback] staking_pool_account: StakingPoolAccount,
    ) -> Promise {
        // the batch should always be present because the purpose of this callback is a step
        // in the batch processing workflow
        // - if the callback was called by itself, and the batch is not present, then there is a bug
        let batch = self.stake_batch.expect(STAKE_BATCH_SHOULD_EXIST);

        let is_liquidity_needed = self.is_liquidity_needed();
        let unstaked_balance = staking_pool_account.unstaked_balance.0;
        if unstaked_balance > 0 && is_liquidity_needed {
            self.add_liquidity_then_deposit_and_stake(unstaked_balance, batch)
        } else {
            // if liquidity is not needed, then stake it
            let stake_amount = if is_liquidity_needed {
                let near_liquidity = self.near_liquidity_pool;
                self.near_liquidity_pool = 0.into();
                batch.balance().amount() + near_liquidity
            } else {
                batch.balance().amount()
            };

            self.staking_pool_promise()
                .deposit_and_stake(stake_amount)
                .get_account()
                .promise()
                .then(self.invoke_on_deposit_and_stake(None))
        }
    }

    /// ## Workflow
    /// 1. if liquidity was added, then update liquidity balance
    ///    - if enough liquidity was added to cover the pending withdrawal, then clear the
    ///      [RedeemLock](crate::domain::RedeemLock)
    /// 2. mint STAKE for the NEAR that was staked
    /// 3. update STAKE token value
    /// 4. create [StakeBatchReceipt](crate::domain::StakeBatchReceipt)
    ///    - [Staked](crate::interface::staking_service::events::Staked) event is logged
    /// 5. pop the [StakeBatch](crate::domain::StakeBatch)
    ///
    /// ## Panics
    /// - if not called by self
    /// - if [StakeBatch](crate::domain::StakeBatch) does not exist
    /// - if any of the upstream Promises failed
    #[private]
    pub fn on_deposit_and_stake(
        &mut self,
        near_liquidity: Option<interface::YoctoNear>,
        #[callback] staking_pool_account: StakingPoolAccount,
    ) -> Promise {
        self.stake_batch_lock = Some(StakeLock::Staked {
            near_liquidity: near_liquidity.map(Into::into),
            staked_balance: staking_pool_account.staked_balance.0.into(),
            unstaked_balance: staking_pool_account.unstaked_balance.0.into(),
        });
        self.invoke_process_stake_batch()
    }

    /// ## Workflow
    /// 1. if liquidity was added, then update liquidity balance
    ///    - if enough liquidity was added to cover the pending withdrawal, then clear the
    ///      [RedeemLock](crate::domain::RedeemLock)
    /// 2. mint STAKE for the NEAR that was staked
    /// 3. update STAKE token value
    /// 4. create [StakeBatchReceipt](crate::domain::StakeBatchReceipt)
    ///    - [Staked](crate::interface::staking_service::events::Staked) event is logged
    /// 5. pop the [StakeBatch](crate::domain::StakeBatch)
    ///
    /// ## Panics
    /// - if not called by self
    /// - if [StakeBatch](crate::domain::StakeBatch) does not exist
    /// - if any of the upstream Promises failed
    #[private]
    pub fn process_staked_batch(&mut self) {
        let batch = self.stake_batch.take().expect(STAKE_BATCH_SHOULD_EXIST);

        if let Some(StakeLock::Staked {
            near_liquidity,
            staked_balance,
            unstaked_balance,
        }) = self.stake_batch_lock
        {
            if let Some(near_liquidity) = near_liquidity {
                if near_liquidity.value() > 0 {
                    *self.near_liquidity_pool += near_liquidity.value();
                    log(NearLiquidityAdded {
                        amount: near_liquidity.value(),
                        balance: self.near_liquidity_pool.value(),
                    });

                    // check if liquidity can clear the pending withdrawal
                    if let Some(receipt) = self.get_pending_withdrawal() {
                        let stake_near_value = receipt.stake_near_value();
                        if self.near_liquidity_pool >= stake_near_value {
                            if let Some(batch) = self.redeem_stake_batch.as_ref() {
                                log(PendingWithdrawalCleared::new(batch, &receipt));
                            }
                            // move the liquidity to the contract's NEAR balance to make it available for withdrawal
                            self.near_liquidity_pool -= stake_near_value;
                            self.total_near.credit(stake_near_value);
                            self.redeem_stake_batch_lock = None;
                            self.pop_redeem_stake_batch();
                        }
                    }
                }
            }

            self.mint_stake_and_update_stake_token_value(staked_balance, unstaked_balance, batch);
            self.create_stake_batch_receipt(batch);
            self.pop_stake_batch();
            self.stake_batch_lock = None
        } else {
            panic!("ERROR: illegal state - should only be called when StakeLock::Staked - current state is: {:?}", self.stake_batch_lock);
        }
    }
}

impl StakeTokenContract {
    pub fn mint_stake_and_update_stake_token_value(
        &mut self,
        staked_balance: YoctoNear,
        unstaked_balance: YoctoNear,
        batch: StakeBatch,
    ) {
        let staked_balance = self.staked_near_balance(staked_balance, unstaked_balance);
        // this is minted using the prior STAKE token value - however, if rewards were issued, then
        // the STAKE token value is stale
        let stake_minted_amount = self.mint_stake(batch);
        self.update_stake_token_value(staked_balance.into());
        // recompute the batch STAKE value using the updated staked NEAR balance
        let batch_stake_value = self
            .stake_token_value
            .near_to_stake(batch.balance().amount());
        // align the total STAKE supply with the STAKE tokens that were minted for the batch
        {
            self.total_stake.debit(stake_minted_amount);
            self.total_stake.credit(batch_stake_value);
            self.update_stake_token_value(staked_balance.into());
        }
    }

    /// the staked NEAR balance is total amount of NEAR deposited and staked in the staking pool
    /// - it's not straight forward because of how staking works: the staking pool will convert
    ///   the deposited NEAR into shares. Because of rounding, not all NEAR may get staked, and the
    ///   remainder will remain unstaked. Thus, we need to take this into account when computing the
    ///   STAKE token value.
    ///
    ///   For example, assume the stake account has no balance to start with.
    ///   When 1 NEAR is deposited and staked, 7 yoctoNEAR will remain unstaked. In this case we
    ///   want to use the total balance (staked + unstaked).
    ///
    /// - when there is a pending withdrawal, it gets a bit more complicated because we don't want to
    ///   count the NEAR that was unstaked due to STAKE that was redeemed. In this case we need to
    ///   subtract the amount that is pending withdrawal and add back in any liquidity (because liquidity
    ///   is derived from restaking unstaked NEAR)
    pub(crate) fn staked_near_balance(
        &self,
        staked_balance: YoctoNear,
        unstaked_balance: YoctoNear,
    ) -> YoctoNear {
        if staked_balance.value() == 0 {
            return 0.into();
        }
        let balance = match self.get_pending_withdrawal() {
            Some(receipt) => {
                staked_balance.value() + unstaked_balance.value()
                    - receipt.stake_near_value().value()
                    + self.near_liquidity_pool.value()
            }
            _ => staked_balance.value() + unstaked_balance.value(),
        };
        balance.into()
    }

    pub(crate) fn is_liquidity_needed(&self) -> bool {
        match self.get_pending_withdrawal() {
            None => false,
            Some(receipt) => receipt.stake_near_value() > self.near_liquidity_pool,
        }
    }

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

        let deposit_amount = batch.balance().amount().value() - near_liquidity;
        if deposit_amount > 0 {
            self.staking_pool_promise()
                .deposit_then_stake(deposit_amount.into(), batch.balance().amount())
                .get_account()
                .promise()
                .then(self.invoke_on_deposit_and_stake(Some(near_liquidity.into())))
        } else {
            self.staking_pool_promise()
                .stake(batch.balance().amount())
                .get_account()
                .promise()
                .then(self.invoke_on_deposit_and_stake(Some(near_liquidity.into())))
        }
    }

    /// creates a create for the batch and saves it to storage
    /// - [Staked](crate::interface::staking_service::events::Staked) event is logged
    fn create_stake_batch_receipt(&mut self, batch: domain::StakeBatch) {
        let stake_batch_receipt =
            domain::StakeBatchReceipt::new(batch.balance().amount(), self.stake_token_value);
        self.stake_batch_receipts
            .insert(&batch.id(), &stake_batch_receipt);

        log(Staked::new(batch.id(), &stake_batch_receipt));
    }

    /// mints new STAKE from the batch using the [stake_token_value] and updates the total STAKE supply
    fn mint_stake(&mut self, batch: domain::StakeBatch) -> YoctoStake {
        let stake_amount = self
            .stake_token_value
            .near_to_stake(batch.balance().amount());
        self.total_stake.credit(stake_amount);
        stake_amount
    }

    /// moves the next batch into the current batch
    fn pop_stake_batch(&mut self) {
        self.stake_batch = self.next_stake_batch.take();
    }
}

/// staking NEAR workflow callback invocations
impl StakeTokenContract {
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
        ext_staking_workflow_callbacks::clear_stake_batch_lock(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config.gas_config().callbacks().unlock().value(),
        )
    }

    pub(crate) fn invoke_on_deposit_and_stake(&self, near_liquidity: Option<YoctoNear>) -> Promise {
        ext_staking_workflow_callbacks::on_deposit_and_stake(
            near_liquidity.map(Into::into),
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .callbacks()
                .on_deposit_and_stake()
                .value(),
        )
    }

    pub(crate) fn invoke_process_stake_batch(&self) -> Promise {
        ext_staking_workflow_callbacks::process_staked_batch(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            // pass on remaining gas
            env::prepaid_gas()
                - env::used_gas()
                - self.config.gas_config().function_call_promise().value()
                - TGAS.value(),
        )
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod test {

    use super::*;
    use crate::contract::staking_pool::GetAccountArgs;
    use crate::{
        interface::{AccountManagement, StakingService},
        near::YOCTO,
        test_utils::*,
    };
    use near_sdk::{serde_json, test_utils::*, testing_env, MockedBlockchain};

    /// Given no liquidity is needed
    /// When the callback is invoked
    /// And the batch funds are deposited and staked with the staking pool
    /// And a callback is scheduled to run once the deposit and stake promise completes
    #[test]
    fn on_run_stake_batch_success() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

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

        let receipts: Vec<Receipt> = deserialize_receipts();
        assert_eq!(receipts.len(), 2);

        {
            let receipt = &receipts[0];
            assert_eq!(receipt.actions.len(), 2);
            {
                let action = &receipt.actions[0];
                match action {
                    Action::FunctionCall {
                        method_name,
                        deposit,
                        gas,
                        ..
                    } => {
                        assert_eq!(method_name, "deposit_and_stake");
                        assert_eq!(*deposit, context.attached_deposit);
                        assert_eq!(
                            *gas,
                            contract
                                .config
                                .gas_config()
                                .staking_pool()
                                .deposit_and_stake()
                                .value()
                        );
                    }
                    _ => panic!("expected `deposit_and_stake` func call"),
                }
            }
            {
                let action = &receipt.actions[1];
                match action {
                    Action::FunctionCall {
                        method_name,
                        deposit,
                        gas,
                        ..
                    } => {
                        assert_eq!(method_name, "get_account");
                        assert_eq!(*deposit, 0);
                        assert_eq!(
                            *gas,
                            contract
                                .config
                                .gas_config()
                                .staking_pool()
                                .get_account()
                                .value()
                        );
                    }
                    _ => panic!("expected `get_account` func call"),
                }
            }
        }
        {
            let receipt = &receipts[1];
            let action = &receipt.actions[0];
            match action {
                Action::FunctionCall {
                    method_name,
                    gas,
                    deposit,
                    ..
                } => {
                    assert_eq!(method_name, "on_deposit_and_stake");
                    assert_eq!(*deposit, 0);
                    assert_eq!(
                        *gas,
                        contract
                            .config
                            .gas_config()
                            .callbacks()
                            .on_deposit_and_stake()
                            .value()
                    );
                }
                _ => panic!("expected `on_deposit_and_stake` func call"),
            }
        }
    }

    /// Given there is a pending withdrawal
    /// And the amount of unstaked NEAR is more than is being staked
    /// When the callback is invoked
    /// Then the entire stake batch NEAR amount is added to the liquidity pool
    /// And a stake request is submitted to the staking pool
    #[test]
    fn on_run_stake_batch_success_with_pending_withdrawal_with_all_near_added_to_liquidity() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

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

        // Given there is a pending withdrawal
        contract.redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        *contract.batch_id_sequence += 1;
        let redeem_stake_batch =
            domain::RedeemStakeBatch::new(contract.batch_id_sequence, (10 * YOCTO).into());
        let receipt = redeem_stake_batch.create_receipt(contract.stake_token_value);
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract
            .redeem_stake_batch_receipts
            .insert(&redeem_stake_batch.id(), &receipt);

        // And the amount of unstaked NEAR is more than is being staked
        let staking_pool_account = StakingPoolAccount {
            account_id: context.predecessor_account_id.clone(),
            unstaked_balance: (200 * YOCTO).into(),
            staked_balance: 0.into(),
            can_withdraw: true,
        };
        contract.on_run_stake_batch(staking_pool_account);
        assert_eq!(contract.near_liquidity_pool.value(), 0);

        let receipts: Vec<Receipt> = deserialize_receipts();
        assert_eq!(receipts.len(), 2);

        {
            let receipt = &receipts[0];
            assert_eq!(receipt.actions.len(), 2);
            {
                let action = &receipt.actions[0];
                if let Action::FunctionCall {
                    method_name,
                    deposit,
                    gas,
                    args,
                } = action
                {
                    assert_eq!(method_name, "stake");
                    assert_eq!(*deposit, 0);
                    assert_eq!(
                        *gas,
                        contract.config.gas_config().staking_pool().stake().value()
                    );
                    let args: StakeArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(
                        args.amount.0,
                        contract.stake_batch.unwrap().balance().amount().value()
                    );
                } else {
                    panic!("expected stake function call")
                }
            }
            {
                let action = &receipt.actions[1];
                if let Action::FunctionCall {
                    method_name,
                    deposit,
                    gas,
                    args,
                } = action
                {
                    assert_eq!(method_name, "get_account");
                    assert_eq!(*deposit, 0);
                    assert_eq!(
                        *gas,
                        contract
                            .config
                            .gas_config()
                            .staking_pool()
                            .get_account()
                            .value()
                    );
                    let args: GetAccountArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(args.account_id, env::current_account_id());
                } else {
                    panic!("expected get_account function call")
                }
            }
        }
        {
            let receipt = &receipts[1];
            let action = &receipt.actions[0];
            if let Action::FunctionCall {
                method_name,
                deposit,
                gas,
                ..
            } = action
            {
                assert_eq!(method_name, "on_deposit_and_stake");
                assert_eq!(*deposit, 0);
                assert_eq!(
                    *gas,
                    contract
                        .config
                        .gas_config()
                        .callbacks()
                        .on_deposit_and_stake()
                        .value()
                );
            } else {
                panic!("expected on_deposit_and_stake function call")
            }
        }
    }

    // Given there is a pending withdrawal
    // And the amount of unstaked NEAR is less than what is being staked
    // When the callback is invoked
    // Then the entire partial batch NEAR amount is added to the liquidity pool
    // And a deposit request and then a stake request are submitted to the staking pool
    #[test]
    fn on_run_stake_batch_success_with_pending_withdrawal_with_partial_near_added_to_liquidity() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        // account deposits 100 NEAR into stake batch
        contract.deposit();
        contract.stake();

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        context.attached_deposit = 100 * YOCTO;
        context.epoch_height += 1;
        testing_env!(context.clone());

        // Given there is a pending withdrawal for 10 NEAR
        {
            contract.redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
            *contract.batch_id_sequence += 1;
            let redeem_stake_batch =
                domain::RedeemStakeBatch::new(contract.batch_id_sequence, (10 * YOCTO).into());
            let receipt = redeem_stake_batch.create_receipt(contract.stake_token_value);
            contract.redeem_stake_batch = Some(redeem_stake_batch);
            contract
                .redeem_stake_batch_receipts
                .insert(&redeem_stake_batch.id(), &receipt);
        }

        // And the amount of unstaked NEAR is less than what is being staked
        let staking_pool_account = StakingPoolAccount {
            account_id: context.predecessor_account_id.clone(),
            unstaked_balance: (40 * YOCTO).into(),
            staked_balance: 0.into(),
            can_withdraw: true,
        };
        // When the callback is invoked
        contract.on_run_stake_batch(staking_pool_account.clone());

        // liquidity should not be persisted to state until after the NEAR funds have been staked
        // successfully with the staking pool
        assert_eq!(contract.near_liquidity_pool.value(), 0);

        let receipts: Vec<Receipt> = deserialize_receipts();
        assert_eq!(receipts.len(), 2);

        {
            let receipt = &receipts[0];
            assert_eq!(receipt.actions.len(), 3);
            {
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
                            .deposit()
                            .value()
                    )
                } else {
                    panic!("expected deposit function call")
                }
            }
            {
                let action = &receipt.actions[1];
                if let Action::FunctionCall {
                    method_name,
                    deposit,
                    gas,
                    args,
                } = action
                {
                    assert_eq!(method_name, "stake");
                    let args: StakeArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(*deposit, 0);
                    assert_eq!(
                        args.amount.0,
                        contract.stake_batch.unwrap().balance().amount().value()
                    );
                    assert_eq!(
                        *gas,
                        contract.config.gas_config().staking_pool().stake().value()
                    )
                } else {
                    panic!("expected stake function call")
                }
            }
            {
                let action = &receipt.actions[2];
                if let Action::FunctionCall {
                    method_name,
                    deposit,
                    gas,
                    ..
                } = action
                {
                    assert_eq!(method_name, "get_account");
                    assert_eq!(*deposit, 0);
                    assert_eq!(
                        *gas,
                        contract
                            .config
                            .gas_config()
                            .staking_pool()
                            .get_account()
                            .value()
                    )
                } else {
                    panic!("expected get_account function call")
                }
            }
        }
        {
            let receipt = &receipts[1];
            let action = &receipt.actions[0];
            if let Action::FunctionCall {
                method_name,
                deposit,
                gas,
                ..
            } = action
            {
                assert_eq!(method_name, "on_deposit_and_stake");
                assert_eq!(*deposit, 0);
                assert_eq!(
                    *gas,
                    contract
                        .config
                        .gas_config()
                        .callbacks()
                        .on_deposit_and_stake()
                        .value()
                )
            } else {
                panic!("expected check_deposit function call")
            }
        }
    }
}
