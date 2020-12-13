use crate::{
    core::Hash,
    domain,
    domain::{Account, StakeBatch, StakeBatchReceipt},
    interface::{
        BatchId, RedeemStakeBatchReceipt, StakeTokenValue, StakingService, YoctoNear, YoctoStake,
    },
    near::NO_DEPOSIT,
    StakeTokenContract,
};
use near_sdk::{
    env, ext_contract,
    json_types::{U128, U64},
    near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId, Gas, Promise, PromiseOrValue,
};

use std::convert::TryFrom;

#[near_bindgen]
impl StakingService for StakeTokenContract {
    fn staking_pool_id(&self) -> AccountId {
        self.staking_pool_id.clone()
    }

    fn deposit(&mut self) -> BatchId {
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = self
            .accounts
            .get(&account_hash)
            .expect("account is not registered");

        assert!(
            env::attached_deposit() > 0,
            "deposit is required in order to stake"
        );

        let batch_id = self.apply_stake_batch_credit(&mut account, env::attached_deposit().into());
        self.save_account(&account_hash, &account);
        batch_id
    }

    fn run_stake_batch(&mut self) -> PromiseOrValue<Option<BatchId>> {
        if self.stake_batch.is_none() {
            return PromiseOrValue::Value(None);
        }

        assert!(!self.locked, "contract is locked");

        self.locked = true;

        let get_staked_balance_gas = self
            .config
            .gas_config()
            .staking_pool()
            .get_account_balance()
            .value();
        let gas_needed_to_complete_this_func_call =
            self.config.gas_config().run_stake_batch().value();
        // give the remainder of the gas to the callback minus what's needed to complete this func call
        let callback_gas = env::prepaid_gas()
            - env::used_gas()
            - get_staked_balance_gas
            - gas_needed_to_complete_this_func_call;

        ext_staking_pool::get_account_staked_balance(
            env::current_account_id(),
            &self.staking_pool_id,
            NO_DEPOSIT.into(),
            get_staked_balance_gas,
        )
        .then(
            ext_staking_pool_callbacks::on_get_account_staked_balance_to_run_stake_batch(
                &env::current_account_id(),
                NO_DEPOSIT.into(),
                callback_gas,
            ),
        )
        .into()
    }

    fn redeem(&mut self, amount: YoctoStake) -> PromiseOrValue<BatchId> {
        unimplemented!()
    }

    fn redeem_all(&mut self) -> PromiseOrValue<BatchId> {
        unimplemented!()
    }

    fn cancel_pending_redeem_stake_request(&mut self) -> bool {
        unimplemented!()
    }

    fn run_redeem_stake_batch(&mut self) -> PromiseOrValue<Option<BatchId>> {
        unimplemented!()
    }

    fn claim_all_batch_receipt_funds(&mut self) {
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = self
            .accounts
            .get(&account_hash)
            .expect("account is not registered");

        if self.claim_receipt_funds(&mut account) {
            self.accounts.insert(&account_hash, &account);
        }
    }

    fn pending_redeem_stake_batch_receipt(&self) -> Option<RedeemStakeBatchReceipt> {
        unimplemented!()
    }

    fn stake_token_value(&self) -> PromiseOrValue<StakeTokenValue> {
        if self.stake_token_value.is_current() {
            return PromiseOrValue::Value(self.stake_token_value.into());
        }

        self.refresh_stake_token_value().into()
    }

    fn refresh_stake_token_value(&self) -> Promise {
        ext_staking_pool::get_account_staked_balance(
            env::current_account_id(),
            &self.staking_pool_id,
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .staking_pool()
                .get_account_balance()
                .value(),
        )
        .then(ext_staking_pool_callbacks::on_get_account_staked_balance(
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .callbacks()
                .on_get_account_staked_balance()
                .value(),
        ))
    }
}

impl StakeTokenContract {
    /// batches the NEAR to stake at the contract level and account level
    /// - if the account has a pre-existing batch, then check the batch's status, i.e., check if
    ///   a batch has a receipt to claim STAKE tokens
    ///   - if STAKE tokens are all claimed on the batch receipt, then delete the batch receipt
    ///
    /// ## Panics
    /// if [amount] is zero
    fn apply_stake_batch_credit(
        &mut self,
        account: &mut Account,
        amount: domain::YoctoNear,
    ) -> BatchId {
        assert_ne!(amount.value(), 0, "amount must not be zero");

        self.claim_stake_batch_receipts(account);

        let batch_id = if self.locked {
            // deposit the funds in the next batch
            let mut batch = self.next_stake_batch.unwrap_or_else(|| {
                // create the next batch
                *self.batch_id_sequence += 1;
                StakeBatch::new(self.batch_id_sequence, domain::YoctoNear(0))
            });
            batch.add(amount);
            self.next_stake_batch = Some(batch);

            let mut batch = account
                .next_stake_batch
                .unwrap_or_else(|| StakeBatch::new(self.batch_id_sequence, domain::YoctoNear(0)));
            batch.add(amount);
            account.next_stake_batch = Some(batch);
            batch.id()
        } else {
            // deposit the funds in the current batch
            let mut batch = self.stake_batch.unwrap_or_else(|| {
                // create the next batch
                *self.batch_id_sequence += 1;
                StakeBatch::new(self.batch_id_sequence, domain::YoctoNear(0))
            });
            batch.add(amount);
            self.stake_batch = Some(batch);

            let mut batch = account
                .stake_batch
                .unwrap_or_else(|| StakeBatch::new(self.batch_id_sequence, domain::YoctoNear(0)));
            batch.add(amount);
            account.stake_batch = Some(batch);
            batch.id()
        };

        batch_id.into()
    }

    /// returns true if funds were claimed, which means the account's state has changed and requires
    /// to be persisted for the changes to take effect
    fn claim_receipt_funds(&mut self, account: &mut Account) -> bool {
        let claimed_stake_tokens = self.claim_stake_batch_receipts(account);
        let claimed_neat_tokens = self.claim_redeem_stake_batch_receipts(account);
        claimed_stake_tokens || claimed_neat_tokens
    }

    fn claim_stake_batch_receipts(&mut self, account: &mut Account) -> bool {
        fn claim_stake_tokens_for_batch(
            contract: &mut StakeTokenContract,
            account: &mut Account,
            batch: StakeBatch,
            mut receipt: domain::StakeBatchReceipt,
        ) {
            // how much NEAR did the account stake in the batch
            let staked_near = batch.balance().balance();

            // claim the STAKE tokens for the account
            let stake = receipt.stake_token_value().near_to_stake(staked_near);
            account.apply_stake_credit(stake);

            // track that the STAKE tokens were claimed
            receipt.stake_tokens_issued(staked_near);
            if receipt.all_claimed() {
                // then delete the receipt and free the storage
                contract.stake_batch_receipts.remove(&batch.id());
            } else {
                contract.stake_batch_receipts.insert(&batch.id(), &receipt);
            }
        }

        let mut claimed_funds = false;

        if let Some(batch) = account.stake_batch {
            if let Some(receipt) = self.stake_batch_receipts.get(&batch.id()) {
                claim_stake_tokens_for_batch(self, account, batch, receipt);
                account.stake_batch = None;
                claimed_funds = true;
            }
        }

        if let Some(batch) = account.next_stake_batch {
            if let Some(receipt) = self.stake_batch_receipts.get(&batch.id()) {
                claim_stake_tokens_for_batch(self, account, batch, receipt);
                account.next_stake_batch = None;
                claimed_funds = true;
            }
        }

        claimed_funds
    }

    /// claim NEAR tokens for redeeming STAKE
    fn claim_redeem_stake_batch_receipts(&mut self, account: &mut Account) -> bool {
        fn claim_redeemed_stake_for_batch(
            contract: &mut StakeTokenContract,
            account: &mut Account,
            batch: domain::RedeemStakeBatch,
            mut receipt: domain::RedeemStakeBatchReceipt,
        ) {
            // how much NEAR did the account stake in the batch
            let redeemed_stake = batch.balance().balance();

            // claim the STAKE tokens for the account
            let near = receipt.stake_token_value().stake_to_near(redeemed_stake);
            account.apply_near_credit(near);

            // track that the STAKE tokens were claimed
            receipt.stake_tokens_redeemed(redeemed_stake);
            if receipt.all_claimed() {
                // then delete the receipt and free the storage
                contract.redeem_stake_batch_receipts.remove(&batch.id());
            } else {
                contract
                    .redeem_stake_batch_receipts
                    .insert(&batch.id(), &receipt);
            }
        }

        let mut claimed_funds = false;

        if let Some(batch) = account.redeem_stake_batch {
            if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                claim_redeemed_stake_for_batch(self, account, batch, receipt);
                account.redeem_stake_batch = None;
                claimed_funds = true;
            }
        }

        if let Some(batch) = account.next_redeem_stake_batch {
            if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                claim_redeemed_stake_for_batch(self, account, batch, receipt);
                account.next_redeem_stake_batch = None;
                claimed_funds = true;
            }
        }

        claimed_funds
    }
}

type Balance = U128;

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn deposit_and_stake(&mut self);

    fn get_account_staked_balance(&self, account_id: AccountId) -> Balance;
}

#[ext_contract(ext_staking_pool_callbacks)]
pub trait ExtStakingPoolCallbacks {
    fn on_deposit_and_stake(
        &mut self,
        staked_balance: Balance,
    ) -> Result<StakeBatchReceipt, RunStakeBatchFailure>;

    fn on_get_account_staked_balance(&self, #[callback] staked_balance: Balance)
        -> StakeTokenValue;

    fn on_refresh_account_staked_balance(
        &mut self,
        #[callback] staked_balance: Balance,
    ) -> StakeTokenValue;

    fn on_get_account_staked_balance_to_run_stake_batch(
        &mut self,
        #[callback] staked_balance: Balance,
    ) -> PromiseOrValue<Result<StakeBatchReceipt, RunStakeBatchFailure>>;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub enum RunStakeBatchFailure {
    GetStakedBalanceFailure(BatchId),
    DepositAndStakeFailure(BatchId),
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::Config;
    use crate::domain::StakeBatchReceipt;
    use crate::interface::AccountManagement;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::json_types::ValidAccountId;
    use near_sdk::{serde_json, testing_env, AccountId, MockedBlockchain, VMContext};
    use std::convert::{TryFrom, TryInto};

    fn operator_id() -> AccountId {
        "operator.stake.oysterpack.near".to_string()
    }

    /// Given the contract is not locked
    /// When an account deposits funds to be staked
    /// Then the funds are deposited into the current stake batch on the account
    /// And the funds are deposited into the current stake batch on the contract
    #[test]
    fn deposit_contract_not_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();

        // Then the funds are deposited into the current stake batch on the account
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(
            stake_batch.balance.balance.value(),
            context.attached_deposit
        );
        assert_eq!(stake_batch.id, batch_id);
        assert!(account.next_stake_batch.is_none());

        // And the funds are deposited into the current stake batch on the contract
        assert_eq!(
            contract.stake_batch.unwrap().balance().balance(),
            context.attached_deposit.into()
        );
        assert!(contract.next_stake_batch.is_none());
    }

    /// Given the contract is locked
    /// When an account deposits funds to be staked
    /// Then the funds are deposited into the next stake batch on the account
    /// And the funds are deposited into the next stake batch on the contract
    #[test]
    fn deposit_contract_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();
        contract.locked = true;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        let stake_batch = account.next_stake_batch.unwrap();
        assert_eq!(
            stake_batch.balance.balance.value(),
            context.attached_deposit
        );
        assert_eq!(stake_batch.id, batch_id);
        assert!(account.stake_batch.is_none());

        // And the funds are deposited into the next stake batch on the contract
        assert_eq!(
            contract.next_stake_batch.unwrap().balance().balance(),
            context.attached_deposit.into()
        );
        assert!(contract.stake_batch.is_none());
    }

    /// Given the contract is not locked
    /// When the account deposits funds to be staked
    /// Then the funds are deposited into the current stake batch
    /// Given the contract is then locked
    /// When the account deposits funds to be staked
    /// Then the funds are deposited into the next stake batch
    /// And both the contract and account have funds in the current and next stake batches
    #[test]
    fn deposit_contract_not_locked_and_then_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert!(account.next_stake_batch.is_none());
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(
            stake_batch.balance.balance.value(),
            context.attached_deposit
        );
        assert_eq!(stake_batch.id, batch_id);

        assert!(contract.next_stake_batch.is_none());
        assert_eq!(
            contract.stake_batch.unwrap().balance().balance(),
            context.attached_deposit.into()
        );

        contract.locked = true;

        context.attached_deposit = 50 * YOCTO;
        testing_env!(context.clone());

        let next_batch_id = contract.deposit();
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(account.stake_batch.unwrap().id, batch_id);
        let next_stake_batch = account.next_stake_batch.unwrap();
        assert_eq!(
            next_stake_batch.balance.balance.value(),
            context.attached_deposit
        );
        assert_eq!(next_stake_batch.id, next_batch_id);

        assert_eq!(contract.stake_batch.unwrap().id().value(), batch_id.0 .0);
        assert_eq!(
            contract.next_stake_batch.unwrap().id().value(),
            next_batch_id.0 .0
        );
    }

    /// Given the account has no funds in stake batches
    /// When funds are claimed
    /// Then there should be no effect
    #[test]
    fn claim_all_batch_receipt_funds_with_no_batched_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        // should have no effect because there are no batches and no receipts
        contract.claim_all_batch_receipt_funds();
    }

    /// Given the account has funds in the stake batch
    /// And there is no receipt for the batch
    /// When funds are claimed
    /// Then there should be no effect on the account
    #[test]
    fn claim_all_batch_receipt_funds_with_funds_in_stake_batch_and_no_receipt() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.apply_stake_batch_credit(&mut account, YOCTO.into());
        contract.save_account(&account_hash, &account);

        // When batch receipts are claimed
        contract.claim_all_batch_receipt_funds();
        // Then there should be no effect on the account
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(stake_batch.id, batch_id);
        assert_eq!(stake_batch.balance.balance, YOCTO.into());
    }

    /// Given the account has funds in the stake batch
    /// And there is a receipt for the batch with additional funds batched into it
    /// When funds are claimed
    /// Then the STAKE tokens should be credited to the account
    /// And the receipt NEAR balance should have been debited
    #[test]
    fn claim_all_batch_receipt_funds_with_funds_in_stake_batch_and_with_receipt() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.apply_stake_batch_credit(&mut account, YOCTO.into());
        contract.save_account(&account_hash, &account);

        // Given there is a receipt for the batch
        // And the receipt exists for the stake batch
        // And STAKE token value = 1 NEAR
        let stake_token_value = domain::StakeTokenValue::new(YOCTO.into(), YOCTO.into());
        let receipt = domain::StakeBatchReceipt::new((2 * YOCTO).into(), stake_token_value);
        let batch_id = domain::BatchId(batch_id.into());
        contract.stake_batch_receipts.insert(&batch_id, &receipt);
        // When batch receipts are claimed
        contract.claim_all_batch_receipt_funds();
        // Assert
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(
            account.stake.unwrap().balance.0 .0,
            YOCTO,
            "the funds should have been claimed by the account"
        );
        assert!(
            account.stake_batch.is_none(),
            "stake batch should be set to None"
        );
        let receipt = contract.stake_batch_receipts.get(&batch_id).unwrap();
        assert_eq!(
            receipt.staked_near().value(),
            YOCTO,
            "claiming STAKE tokens should have reduced the near balance on the receipt"
        );

        // Given account has funds deposited into the current StakeBatch
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.apply_stake_batch_credit(&mut account, YOCTO.into());
        contract.save_account(&account_hash, &account);
        // When batch receipts are claimed
        contract.claim_all_batch_receipt_funds();
        // Assert
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(
            account.stake.unwrap().balance.0 .0,
            2 * YOCTO,
            "the funds should have been claimed by the account"
        );
        assert!(
            account.stake_batch.is_none(),
            "stake batch should be set to None"
        );
        let batch_id = domain::BatchId(batch_id.0 .0);
        let receipt = contract.stake_batch_receipts.get(&batch_id);
        assert!(
            receipt.is_none(),
            "when all STAKE tokens are claimed, then the receipt should have been deleted"
        );
    }

    /// Given the account has funds in the stake batch
    /// And there is a receipt for the batch with exact matching funds
    /// When funds are claimed
    /// Then the STAKE tokens should be credited to the account
    /// And the receipt is deleted
    #[test]
    fn claim_all_batch_receipt_funds_with_all_stake_batch_funds_claimed_on_receipt() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.apply_stake_batch_credit(&mut account, (2 * YOCTO).into());
        contract.save_account(&account_hash, &account);

        // Given there is a receipt for the batch
        // And the receipt exists for the stake batch
        // And STAKE token value = 1 NEAR
        let stake_token_value = domain::StakeTokenValue::new(YOCTO.into(), YOCTO.into());
        let receipt = domain::StakeBatchReceipt::new((2 * YOCTO).into(), stake_token_value);
        let batch_id = domain::BatchId(batch_id.into());
        contract.stake_batch_receipts.insert(&batch_id, &receipt);
        // When batch receipts are claimed
        contract.claim_all_batch_receipt_funds();

        // Assert
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(
            account.stake.unwrap().balance.0 .0,
            2 * YOCTO,
            "the funds should have been claimed by the account"
        );
        assert!(
            account.stake_batch.is_none(),
            "stake batch should be set to None"
        );
        let receipt = contract.stake_batch_receipts.get(&batch_id);
        assert!(
            receipt.is_none(),
            "when all STAKE tokens are claimed, then the receipt should have been deleted"
        );
    }

    /// Given Account::stake_batch and Account::next_stake_batch both have funds
    /// And there are exact receipts for both batches
    /// Then STAKE tokens should be claimed for both
    /// And the receipts should be deleted
    #[test]
    fn claim_all_batch_receipt_funds_with_stake_batch_and_next_stake_batch_funds_with_receipts() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let stake_batch_id = contract.apply_stake_batch_credit(&mut account, (2 * YOCTO).into());
        assert_eq!(
            contract.stake_batch.unwrap().balance().balance(),
            (2 * YOCTO).into()
        );
        // locking the contract should deposit the funds into the next stake batch
        contract.locked = true;
        let next_stake_batch_id =
            contract.apply_stake_batch_credit(&mut account, (3 * YOCTO).into());
        assert_eq!(
            contract.next_stake_batch.unwrap().balance().balance(),
            (3 * YOCTO).into()
        );
        contract.save_account(&account_hash, &account);

        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(
            account.stake_batch.unwrap().balance.balance.value(),
            2 * YOCTO
        );
        assert_eq!(
            account.next_stake_batch.unwrap().balance.balance.value(),
            3 * YOCTO
        );

        contract.locked = false;

        // Given that the batches have receipts
        // And STAKE token value = 1 NEAR
        let stake_token_value = domain::StakeTokenValue::new(YOCTO.into(), YOCTO.into());
        let receipt = domain::StakeBatchReceipt::new((2 * YOCTO).into(), stake_token_value);
        contract
            .stake_batch_receipts
            .insert(&domain::BatchId(stake_batch_id.into()), &receipt);
        let receipt = domain::StakeBatchReceipt::new((3 * YOCTO).into(), stake_token_value);
        contract
            .stake_batch_receipts
            .insert(&domain::BatchId(next_stake_batch_id.into()), &receipt);
        // When batch receipts are claimed
        contract.claim_all_batch_receipt_funds();
        // then receipts should be deleted because all funds have been claimed
        assert!(contract.stake_batch_receipts.is_empty());

        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        // and the account batches have been cleared
        assert!(account.stake_batch.is_none());
        assert!(account.next_stake_batch.is_none());
        // and the STAKE tokens were claimed and credited to the account
        assert_eq!(account.stake.unwrap().balance.0 .0, 5 * YOCTO);
    }

    /// Given there is no stake batch to run
    /// Then the contract returns None
    #[test]
    fn run_stake_batch_no_stake_batch() {
        let account_id = "alfio-zappala.near";
        let context = new_context(account_id);
        testing_env!(context);

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        match contract.run_stake_batch() {
            PromiseOrValue::Value(None) => (),
            _ => panic!("unexpected result"),
        }
    }

    /// Given the contract has a stake batch
    /// When the stake batch is run
    /// Then the contract is locked
    /// When the stake batch is run again while the contract is locked
    /// Then the func call panics
    #[test]
    #[should_panic(expected = "contract is locked")]
    fn run_stake_batch_contract_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        context.attached_deposit = YOCTO;
        contract.deposit();

        contract.run_stake_batch();
        assert!(contract.locked);

        // should panic because contract is locked
        contract.run_stake_batch();
    }

    /// Given the contract has just been deployed
    /// And the STAKE token value is retrieved within the same epoch
    /// Then the cached version should be returned
    #[test]
    fn stake_token_value_is_current() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.epoch_height = 10;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.total_stake.credit(YOCTO.into());
        contract.stake_token_value = domain::StakeTokenValue::new(YOCTO.into(), YOCTO.into());

        if let PromiseOrValue::Value(stake_token_value) = contract.stake_token_value() {
            assert_eq!(
                stake_token_value.total_stake_supply,
                contract.total_stake.balance().into()
            );
            assert_eq!(stake_token_value.total_staked_near_balance, YOCTO.into());
        } else {
            panic!("cached StakeTokenValue should have been returned")
        }
    }

    /// Given the contract has a stake batch
    /// And the contract is not locked
    /// When the stake batch is run
    /// Then it generates to FunctionCall receipts:
    ///   1. to get the staked balance from the staking pool contract
    ///   2. and then to callback into this contract - on_get_account_staked_balance_to_run_stake_batch
    #[test]
    fn run_stake_batch_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings.clone());

        contract.register_account();

        context.attached_deposit = YOCTO;
        contract.deposit();

        // context.prepaid_gas = 10u64.pow(18);
        testing_env!(context.clone());
        contract.run_stake_batch();
        println!(
            "prepaid gas: {}, used_gas: {}, unused_gas: {}",
            context.prepaid_gas,
            env::used_gas(),
            context.prepaid_gas - env::used_gas()
        );

        let txn_receipts = env::created_receipts();
        let receipts: Vec<Receipt> = txn_receipts
            .iter()
            .map(|receipt| {
                let json = serde_json::to_string_pretty(receipt).unwrap();
                println!("{}", json);
                let receipt: Receipt = serde_json::from_str(&json).unwrap();
                receipt
            })
            .collect();
        assert_eq!(txn_receipts.len(), 2);

        // there should be a `get_account_staked_balance` func call on the staking pool
        let _get_staked_balance_func_call = receipts
            .iter()
            .find(|receipt| {
                if receipt.receiver_id.as_str()
                    == contract_settings.staking_pool_id.as_ref().as_str()
                {
                    if let Some(Action::FunctionCall { method_name, .. }) = receipt.actions.first()
                    {
                        method_name == "get_account_staked_balance"
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap();

        // and a callback - `on_get_account_staked_balance_to_run_stake_batch`
        let on_get_account_staked_balance_to_run_stake_batch_func_call = receipts
            .iter()
            .find(|receipt| {
                if receipt.receiver_id == context.current_account_id {
                    if let Some(Action::FunctionCall { method_name, .. }) = receipt.actions.first()
                    {
                        method_name == "on_get_account_staked_balance_to_run_stake_batch"
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap();
        // and the callback requires a data receipt, i.e., the staked balance
        assert_eq!(
            on_get_account_staked_balance_to_run_stake_batch_func_call
                .receipt_indices
                .len(),
            1
        );
        assert_eq!(
            *on_get_account_staked_balance_to_run_stake_batch_func_call
                .receipt_indices
                .first()
                .unwrap(),
            0
        );
    }
}
