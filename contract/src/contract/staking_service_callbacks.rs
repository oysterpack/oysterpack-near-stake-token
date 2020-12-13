use crate::{
    core::Hash,
    domain,
    domain::{Account, StakeBatch},
    ext_staking_pool, ext_staking_pool_callbacks,
    interface::{BatchId, RedeemStakeBatchReceipt, StakingService, YoctoStake},
    interface::{StakeBatchReceipt, StakeTokenValue, YoctoNear},
    near::{assert_predecessor_is_self, is_promise_result_success, NO_DEPOSIT},
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
            is_promise_result_success(env::promise_result(0)),
            "failed to get staked balance from staking pool"
        );
        domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.balance()).into()
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

        if !is_promise_result_success(env::promise_result(0)) {
            self.locked = false;
            return PromiseOrValue::Value(Err(RunStakeBatchFailure::GetStakedBalanceFailure(
                batch.id().into(),
            )));
        }

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

        let deposit_and_stake_succeeded = is_promise_result_success(env::promise_result(0));
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
    use crate::config::Config;
    use crate::domain::StakeBatchReceipt;
    use crate::interface::AccountManagement;
    use crate::near::{is_promise_result_success, YOCTO};
    use crate::test_utils::{
        expected_account_storage_fee, near, Action, Receipt, EXPECTED_ACCOUNT_STORAGE_USAGE,
    };
    use near_sdk::json_types::ValidAccountId;
    use near_sdk::{serde_json, testing_env, AccountId, MockedBlockchain, VMContext};
    use std::convert::TryFrom;

    #[test]
    fn on_get_account_staked_balance_success() {
        unimplemented!()
    }
}
