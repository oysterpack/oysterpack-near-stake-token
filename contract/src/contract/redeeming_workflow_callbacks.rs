use crate::errors::illegal_state::REDEEM_STAKE_BATCH_SHOULD_EXIST;
use crate::errors::redeeming_stake_errors::{
    UNSTAKED_FUNDS_NOT_AVAILABLE_FOR_WITHDRAWAL, UNSTAKING_BLOCKED_BY_PENDING_WITHDRAWAL,
};
use crate::errors::staking_pool_failures::{
    GET_ACCOUNT_FAILURE, GET_STAKED_BALANCE_FAILURE, UNSTAKE_FAILURE,
};
use crate::{
    domain::{self, RedeemLock},
    ext_redeeming_workflow_callbacks, ext_staking_pool,
    interface::{BatchId, Operator},
    near::{assert_predecessor_is_self, NO_DEPOSIT},
    StakeTokenContract, StakingPoolAccount,
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
            .expect("illegal state - batch should exist while pending withdrawal")
            .id();

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
            .take()
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
                .on_run_stake_batch()
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
mod test {}
