//required in order for near_bindgen macro to work outside of lib.rs
use crate::*;
use crate::{
    domain::{self, Account, RedeemLock, RedeemStakeBatch, StakeBatch},
    errors::{
        illegal_state::{REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST, REDEEM_STAKE_BATCH_SHOULD_EXIST},
        redeeming_stake_errors::{NO_REDEEM_STAKE_BATCH_TO_RUN, UNSTAKED_FUNDS_PENDING_WITHDRAWAL},
        staking_errors::{
            BLOCKED_BY_BATCH_RUNNING, NO_FUNDS_IN_STAKE_BATCH_TO_WITHDRAW, NO_STAKE_BATCH_TO_RUN,
        },
        staking_service::{
            DEPOSIT_REQUIRED_FOR_STAKE, INSUFFICIENT_STAKE_FOR_REDEEM_REQUEST, ZERO_REDEEM_AMOUNT,
        },
    },
    interface::{BatchId, RedeemStakeBatchReceipt, StakingService, YoctoNear, YoctoStake},
    near::NO_DEPOSIT,
};
use near_sdk::{
    env, ext_contract, near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId, Promise, PromiseOrValue,
};

#[near_bindgen]
impl StakingService for StakeTokenContract {
    fn staking_pool_id(&self) -> AccountId {
        self.staking_pool_id.clone()
    }

    fn stake_batch_receipt(&self, batch_id: BatchId) -> Option<interface::StakeBatchReceipt> {
        self.stake_batch_receipts
            .get(&batch_id.into())
            .map(interface::StakeBatchReceipt::from)
    }

    fn redeem_stake_batch_receipt(
        &self,
        batch_id: BatchId,
    ) -> Option<interface::RedeemStakeBatchReceipt> {
        self.redeem_stake_batch_receipts
            .get(&batch_id.into())
            .map(interface::RedeemStakeBatchReceipt::from)
    }

    #[payable]
    fn deposit(&mut self) -> BatchId {
        let (mut account, account_id_hash) =
            self.registered_account(&env::predecessor_account_id());

        let batch_id =
            self.deposit_near_for_account_to_stake(&mut account, env::attached_deposit().into());
        self.save_account(&account_id_hash, &account);
        batch_id
    }

    /// runs the stake batch
    ///
    /// logical workflow:
    /// 1. lock the contract
    /// 2. get account from staking pool
    /// 3. deposit and stake NEAR funds
    /// 4. create stake batch receipt
    /// 5. update STAKE token supply
    /// 6. unlock contract
    fn stake(&mut self) -> Promise {
        assert!(self.can_run_batch(), BLOCKED_BY_BATCH_RUNNING);
        assert!(self.stake_batch.is_some(), NO_STAKE_BATCH_TO_RUN);

        self.run_stake_batch_locked = true;

        self.get_account_from_staking_pool() // 5 TGas
            .then(self.invoke_on_run_stake_batch()) // 85 TGas
            .then(self.invoke_release_run_stake_batch_lock()) // 5 TGas
    }

    #[payable]
    fn deposit_and_stake(&mut self) -> PromiseOrValue<BatchId> {
        let batch_id = self.deposit();

        if self.can_run_batch() {
            PromiseOrValue::Promise(self.stake())
        } else {
            PromiseOrValue::Value(batch_id)
        }
    }

    fn withdraw_funds_from_stake_batch(&mut self, amount: YoctoNear) {
        let (mut account, account_id_hash) =
            self.registered_account(&env::predecessor_account_id());
        self.claim_receipt_funds(&mut account);

        if let Some(mut batch) = account.next_stake_batch {
            let amount = amount.into();

            // remove funds from contract level batch
            {
                let mut batch = self.next_stake_batch.expect(
                    "next_stake_batch at contract level should exist if it exists at account level",
                );

                if batch.remove(amount).value() == 0 {
                    self.next_stake_batch = None;
                } else {
                    self.next_stake_batch = Some(batch);
                }
            }

            if batch.remove(amount).value() == 0 {
                account.next_stake_batch = None;
            } else {
                account.next_stake_batch = Some(batch);
            }
            self.save_account(&account_id_hash, &account);
            Promise::new(env::predecessor_account_id()).transfer(amount.value());
            return;
        }

        if let Some(mut batch) = account.stake_batch {
            assert!(self.can_run_batch(), BLOCKED_BY_BATCH_RUNNING);

            let amount = amount.into();

            // remove funds from contract level batch
            {
                let mut batch = self.stake_batch.expect(
                    "stake_batch at contract level should exist if it exists at account level",
                );
                if batch.remove(amount).value() == 0 {
                    self.stake_batch = None;
                } else {
                    self.stake_batch = Some(batch);
                }
            }

            if batch.remove(amount).value() == 0 {
                account.stake_batch = None;
            } else {
                account.stake_batch = Some(batch);
            }
            self.save_account(&account_id_hash, &account);
            Promise::new(env::predecessor_account_id()).transfer(amount.value());
            return;
        }

        panic!(NO_FUNDS_IN_STAKE_BATCH_TO_WITHDRAW);
    }

    fn withdraw_all_funds_from_stake_batch(&mut self) {
        let (mut account, account_hash) = self.registered_account(&env::predecessor_account_id());
        self.claim_receipt_funds(&mut account);

        if let Some(batch) = account.next_stake_batch {
            let amount = batch.balance().amount();

            // remove funds from contract level batch
            {
                let mut batch = self.next_stake_batch.expect(
                    "next_stake_batch at contract level should exist if it exists at account level",
                );
                if batch.remove(amount).value() == 0 {
                    self.next_stake_batch = None;
                } else {
                    self.next_stake_batch = Some(batch);
                }
            }

            account.next_stake_batch = None;
            self.save_account(&account_hash, &account);
            Promise::new(env::predecessor_account_id()).transfer(amount.value());
            return;
        }

        if let Some(batch) = account.stake_batch {
            assert!(self.can_run_batch(), BLOCKED_BY_BATCH_RUNNING);

            let amount = batch.balance().amount();

            // remove funds from contract level batch
            {
                let mut batch = self.stake_batch.expect(
                    "next_stake_batch at contract level should exist if it exists at account level",
                );
                if batch.remove(amount).value() == 0 {
                    self.stake_batch = None;
                } else {
                    self.stake_batch = Some(batch);
                }
            }

            account.stake_batch = None;
            self.save_account(&account_hash, &account);
            Promise::new(env::predecessor_account_id()).transfer(amount.value());
            return;
        }

        panic!(NO_FUNDS_IN_STAKE_BATCH_TO_WITHDRAW);
    }

    fn redeem(&mut self, amount: YoctoStake) -> BatchId {
        let (mut account, account_id_hash) =
            self.registered_account(&env::predecessor_account_id());
        let batch_id = self.redeem_stake_for_account(&mut account, amount.into());
        self.save_account(&account_id_hash, &account);
        batch_id
    }

    fn redeem_all(&mut self) -> BatchId {
        let (mut account, account_id_hash) =
            self.registered_account(&env::predecessor_account_id());
        self.claim_receipt_funds(&mut account);
        let amount = account.stake.expect("account has no stake").amount();
        let batch_id = self.redeem_stake_for_account(&mut account, amount);
        self.save_account(&account_id_hash, &account);
        batch_id
    }

    fn cancel_uncommitted_redeem_stake_batch(&mut self) -> bool {
        let (mut account, account_id_hash) =
            self.registered_account(&env::predecessor_account_id());
        self.claim_receipt_funds(&mut account);

        if self.run_redeem_stake_batch_lock.is_none() {
            if let Some(batch) = account.redeem_stake_batch {
                let amount = batch.balance().amount();

                // remove funds from contract level batch
                {
                    let mut batch = self.redeem_stake_batch.expect(
                        "redeem_stake_batch at contract level should exist if it exists at account level",
                    );
                    if batch.remove(amount).value() == 0 {
                        self.redeem_stake_batch = None;
                    } else {
                        self.redeem_stake_batch = Some(batch);
                    }
                }

                account.apply_stake_credit(amount);
                account.redeem_stake_batch = None;
                self.save_account(&account_id_hash, &account);
                return true;
            }

            self.save_account(&account_id_hash, &account);
            false
        } else {
            if let Some(batch) = account.next_redeem_stake_batch {
                let amount = batch.balance().amount();

                // remove funds from contract level batch
                {
                    let mut batch = self.next_redeem_stake_batch.expect(
                        "next_redeem_stake_batch at contract level should exist if it exists at account level",
                    );
                    if batch.remove(amount).value() == 0 {
                        self.next_redeem_stake_batch = None;
                    } else {
                        self.next_redeem_stake_batch = Some(batch);
                    }
                }

                account.apply_stake_credit(amount);
                account.next_redeem_stake_batch = None;
                self.save_account(&account_id_hash, &account);
                return true;
            }

            self.save_account(&account_id_hash, &account);
            false
        }
    }

    fn unstake(&mut self) -> Promise {
        assert!(self.can_run_batch(), BLOCKED_BY_BATCH_RUNNING);

        match self.run_redeem_stake_batch_lock {
            None => {
                assert!(
                    self.redeem_stake_batch.is_some(),
                    NO_REDEEM_STAKE_BATCH_TO_RUN
                );
                self.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

                self.get_account_from_staking_pool()
                    .then(self.invoke_on_run_redeem_stake_batch())
                    .then(self.invoke_release_run_redeem_stake_batch_unstaking_lock())
            }
            Some(RedeemLock::PendingWithdrawal) => {
                let batch = self
                    .redeem_stake_batch
                    .expect(REDEEM_STAKE_BATCH_SHOULD_EXIST);
                let batch_receipt = self
                    .redeem_stake_batch_receipts
                    .get(&batch.id())
                    .expect(REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST);
                assert!(
                    batch_receipt.unstaked_funds_available_for_withdrawal(),
                    UNSTAKED_FUNDS_PENDING_WITHDRAWAL
                );

                self.get_account_from_staking_pool()
                    .then(self.invoke_on_redeeming_stake_pending_withdrawal())
            }
            // this should already be handled by above assert and should never be hit
            // but it was added to satisfy the match clause for completeness
            Some(RedeemLock::Unstaking) => panic!(BLOCKED_BY_BATCH_RUNNING),
        }
    }

    fn redeem_and_unstake(&mut self, amount: YoctoStake) -> PromiseOrValue<BatchId> {
        let batch_id = self.redeem(amount);

        if self.can_unstake() {
            PromiseOrValue::Promise(self.unstake())
        } else {
            PromiseOrValue::Value(batch_id)
        }
    }

    fn redeem_all_and_unstake(&mut self) -> PromiseOrValue<BatchId> {
        let batch_id = self.redeem_all();

        if self.can_unstake() {
            PromiseOrValue::Promise(self.unstake())
        } else {
            PromiseOrValue::Value(batch_id)
        }
    }

    fn pending_withdrawal(&self) -> Option<RedeemStakeBatchReceipt> {
        match self.redeem_stake_batch {
            Some(batch) => self
                .redeem_stake_batch_receipts
                .get(&batch.id())
                .map(RedeemStakeBatchReceipt::from),
            None => None,
        }
    }
}

// staking pool func call invocations
impl StakeTokenContract {
    pub(crate) fn get_account_from_staking_pool(&self) -> Promise {
        ext_staking_pool::get_account(
            env::current_account_id(),
            &self.staking_pool_id,
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .staking_pool()
                .get_account()
                .value(),
        )
    }
}

impl StakeTokenContract {
    fn can_run_batch(&self) -> bool {
        !self.run_stake_batch_locked && !self.is_unstaking()
    }

    fn can_unstake(&self) -> bool {
        if self.can_run_batch() {
            match self.run_redeem_stake_batch_lock {
                None => self.redeem_stake_batch.is_some(),
                Some(RedeemLock::PendingWithdrawal) => {
                    let batch = self
                        .redeem_stake_batch
                        .expect(REDEEM_STAKE_BATCH_SHOULD_EXIST);
                    let batch_receipt = self
                        .redeem_stake_batch_receipts
                        .get(&batch.id())
                        .expect(REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST);
                    batch_receipt.unstaked_funds_available_for_withdrawal()
                }
                Some(RedeemLock::Unstaking) => false,
            }
        } else {
            self.can_run_batch()
        }
    }

    /// batches the NEAR to stake at the contract level and account level
    ///
    /// ## Panics
    /// if [amount] is zero
    ///
    /// ## Notes
    /// - before applying the deposit, batch receipts are processed [claim_receipt_funds]
    pub(crate) fn deposit_near_for_account_to_stake(
        &mut self,
        account: &mut Account,
        amount: domain::YoctoNear,
    ) -> BatchId {
        assert!(amount.value() > 0, DEPOSIT_REQUIRED_FOR_STAKE);

        self.claim_receipt_funds(account);

        // use current batch if not staking, i.e., the stake batch is not running
        if !self.run_stake_batch_locked {
            // apply at contract level
            let mut contract_batch = self.stake_batch.unwrap_or_else(|| self.new_stake_batch());
            contract_batch.add(amount);
            self.stake_batch = Some(contract_batch);

            // apply at account level
            // NOTE: account batch ID must match contract batch ID
            let mut account_batch = account
                .stake_batch
                .unwrap_or_else(|| contract_batch.id().new_stake_batch());
            account_batch.add(amount);
            account.stake_batch = Some(account_batch);

            account_batch.id().into()
        } else {
            // apply at contract level
            let mut contract_batch = self
                .next_stake_batch
                .unwrap_or_else(|| self.new_stake_batch());
            contract_batch.add(amount);
            self.next_stake_batch = Some(contract_batch);

            // apply at account level
            // NOTE: account batch ID must match contract batch ID
            let mut account_batch = account
                .next_stake_batch
                .unwrap_or_else(|| contract_batch.id().new_stake_batch());
            account_batch.add(amount);
            account.next_stake_batch = Some(account_batch);

            account_batch.id().into()
        }
    }

    fn new_stake_batch(&mut self) -> StakeBatch {
        *self.batch_id_sequence += 1;
        self.batch_id_sequence.new_stake_batch()
    }

    /// moves STAKE [amount] from account balance to redeem stake batch
    ///
    /// ## Panics
    /// - if amount == 0
    /// - if STAKE account balance is too low to fulfill request
    ///
    /// ## Notes
    /// - before applying the deposit, batch receipts are processed [claim_receipt_funds]
    fn redeem_stake_for_account(
        &mut self,
        account: &mut Account,
        amount: domain::YoctoStake,
    ) -> BatchId {
        assert!(amount.value() > 0, ZERO_REDEEM_AMOUNT);

        self.claim_receipt_funds(account);

        assert!(
            account.can_redeem(amount),
            INSUFFICIENT_STAKE_FOR_REDEEM_REQUEST
        );

        // debit the amount of STAKE to redeem from the account
        let mut stake = account.stake.expect("account has zero STAKE token balance");
        if stake.debit(amount).value() > 0 {
            account.stake = Some(stake);
        } else {
            account.stake = None;
        }

        if self.run_redeem_stake_batch_lock.is_none() {
            // apply at contract level
            let mut contract_batch = self
                .redeem_stake_batch
                .unwrap_or_else(|| self.new_redeem_stake_batch());
            contract_batch.add(amount);
            self.redeem_stake_batch = Some(contract_batch);

            // apply at account level
            // NOTE: account batch ID must match contract batch ID
            let mut account_batch = account
                .redeem_stake_batch
                .unwrap_or_else(|| contract_batch.id().new_redeem_stake_batch());
            account_batch.add(amount);
            account.redeem_stake_batch = Some(account_batch);

            account_batch.id().into()
        } else {
            // apply at contract level
            let mut contract_batch = self
                .next_redeem_stake_batch
                .unwrap_or_else(|| self.new_redeem_stake_batch());
            contract_batch.add(amount);
            self.next_redeem_stake_batch = Some(contract_batch);

            // apply at account level
            // NOTE: account batch ID must match contract batch ID
            let mut account_batch = account
                .next_redeem_stake_batch
                .unwrap_or_else(|| contract_batch.id().new_redeem_stake_batch());
            account_batch.add(amount);
            account.next_redeem_stake_batch = Some(account_batch);

            account_batch.id().into()
        }
    }

    fn new_redeem_stake_batch(&mut self) -> RedeemStakeBatch {
        *self.batch_id_sequence += 1;
        self.batch_id_sequence.new_redeem_stake_batch()
    }

    /// returns true if funds were claimed, which means the account's state has changed and requires
    /// to be persisted for the changes to take effect
    pub(crate) fn claim_receipt_funds(&mut self, account: &mut Account) -> bool {
        let claimed_stake_tokens = self.claim_stake_batch_receipts(account);
        let claimed_neat_tokens = self.claim_redeem_stake_batch_receipts(account);
        claimed_stake_tokens || claimed_neat_tokens
    }

    /// the purpose of this method is to to compute the account's STAKE balance taking into consideration
    /// that there may be unclaimed receipts on the account
    /// - this enables the latest account info to be returned within the context of a contract 'view'
    ///   call - no receipts are physically claimed, i.e., contract state does not change
    pub(crate) fn apply_receipt_funds_for_view(&self, account: &Account) -> Account {
        let mut account = account.clone();

        if let Some(batch) = account.stake_batch {
            if let Some(receipt) = self.stake_batch_receipts.get(&batch.id()) {
                let staked_near = batch.balance().amount();
                let stake = receipt.stake_token_value().near_to_stake(staked_near);
                account.apply_stake_credit(stake);
                account.stake_batch = None;
            }
        }

        if let Some(batch) = account.next_stake_batch {
            if let Some(receipt) = self.stake_batch_receipts.get(&batch.id()) {
                let staked_near = batch.balance().amount();
                let stake = receipt.stake_token_value().near_to_stake(staked_near);
                account.apply_stake_credit(stake);
                account.next_stake_batch = None;
            }
        }

        if let Some(RedeemLock::PendingWithdrawal) = self.run_redeem_stake_batch_lock {
            // NEAR funds cannot be claimed from a receipt that is pending withdrawal from the staking pool
            let batch_pending_withdrawal_id = self.redeem_stake_batch.as_ref().unwrap().id();

            if let Some(batch) = account.redeem_stake_batch {
                if batch_pending_withdrawal_id != batch.id() {
                    if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                        let redeemed_stake = batch.balance().amount();
                        let near = receipt.stake_token_value().stake_to_near(redeemed_stake);
                        account.apply_near_credit(near);
                        account.redeem_stake_batch = None
                    }
                }
            }

            if let Some(batch) = account.next_redeem_stake_batch {
                if batch_pending_withdrawal_id != batch.id() {
                    if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                        let redeemed_stake = batch.balance().amount();
                        let near = receipt.stake_token_value().stake_to_near(redeemed_stake);
                        account.apply_near_credit(near);
                        account.next_redeem_stake_batch = None
                    }
                }
            }
        } else {
            if let Some(batch) = account.redeem_stake_batch {
                if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                    let redeemed_stake = batch.balance().amount();
                    let near = receipt.stake_token_value().stake_to_near(redeemed_stake);
                    account.apply_near_credit(near);
                    account.redeem_stake_batch = None
                }
            }

            if let Some(batch) = account.next_redeem_stake_batch {
                if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                    let redeemed_stake = batch.balance().amount();
                    let near = receipt.stake_token_value().stake_to_near(redeemed_stake);
                    account.apply_near_credit(near);
                    account.next_redeem_stake_batch = None
                }
            }
        }

        account
    }

    fn claim_stake_batch_receipts(&mut self, account: &mut Account) -> bool {
        fn claim_stake_tokens_for_batch(
            contract: &mut StakeTokenContract,
            account: &mut Account,
            batch: StakeBatch,
            mut receipt: domain::StakeBatchReceipt,
        ) {
            // how much NEAR did the account stake in the batch
            let staked_near = batch.balance().amount();

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

        // move the next batch into the current batch as long as the contract is not locked and the
        // funds for the current batch have been claimed
        //
        // NOTE: while the contract is locked for running a stake batch, all deposits must go into the
        // the next batch
        if !self.run_stake_batch_locked && account.stake_batch.is_none() {
            account.stake_batch = account.next_stake_batch.take();
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
            // how much STAKE did the account redeem in the batch
            let redeemed_stake = batch.balance().amount();

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

        fn claim_redeemed_stake_for_batch_pending_withdrawal(
            contract: &mut StakeTokenContract,
            account: &mut Account,
            batch: &mut domain::RedeemStakeBatch,
            mut receipt: domain::RedeemStakeBatchReceipt,
        ) {
            // how much STAKE did the account redeem in the batch
            let redeemed_stake = batch.balance().amount();
            // compute STAKE liquidity
            let stake_liquidity = receipt
                .stake_token_value()
                .near_to_stake(contract.near_liquidity_pool);
            // compute ho much STAKE can be redeemed from liquidity pool
            let redeemable_stake = if stake_liquidity >= redeemed_stake {
                redeemed_stake
            } else {
                stake_liquidity
            };
            batch.remove(redeemable_stake);

            // claim the STAKE tokens for the account
            let near = receipt.stake_token_value().stake_to_near(redeemable_stake);
            account.apply_near_credit(near);
            contract.near_liquidity_pool -= near;
            contract.total_near.credit(near);

            // track that the STAKE tokens were claimed
            receipt.stake_tokens_redeemed(redeemable_stake);
            if receipt.all_claimed() {
                // then delete the receipt and free the storage
                contract.redeem_stake_batch_receipts.remove(&batch.id());
                contract.run_redeem_stake_batch_lock = None;
                contract.pop_redeem_stake_batch();
            } else {
                contract
                    .redeem_stake_batch_receipts
                    .insert(&batch.id(), &receipt);
            }
        }

        let mut claimed_funds = false;

        match self.run_redeem_stake_batch_lock {
            // NEAR funds can be claimed for receipts that are not pending on the unstaked NEAR withdrawal
            // NEAR funds can also be claimed against the NEAR liquidity pool
            Some(RedeemLock::PendingWithdrawal) => {
                // NEAR funds cannot be claimed for a receipt that is pending withdrawal of unstaked NEAR from the staking pool
                let pending_batch_id = self
                    .redeem_stake_batch
                    .expect(REDEEM_STAKE_BATCH_SHOULD_EXIST)
                    .id();

                if let Some(mut batch) = account.redeem_stake_batch {
                    if batch.id() != pending_batch_id {
                        if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                            claim_redeemed_stake_for_batch(self, account, batch, receipt);
                            account.redeem_stake_batch = None;
                            claimed_funds = true;
                        }
                    } else if self.near_liquidity_pool.value() > 0 {
                        if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                            claim_redeemed_stake_for_batch_pending_withdrawal(
                                self, account, &mut batch, receipt,
                            );
                            if batch.balance().amount().value() == 0 {
                                account.redeem_stake_batch = None;
                            } else {
                                account.redeem_stake_batch = Some(batch);
                            }
                            claimed_funds = true;
                        }
                    }
                }

                if let Some(mut batch) = account.next_redeem_stake_batch {
                    if batch.id() != pending_batch_id {
                        if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                            claim_redeemed_stake_for_batch(self, account, batch, receipt);
                            account.next_redeem_stake_batch = None;
                            claimed_funds = true;
                        }
                    } else if self.near_liquidity_pool.value() > 0 {
                        if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                            claim_redeemed_stake_for_batch_pending_withdrawal(
                                self, account, &mut batch, receipt,
                            );
                            if batch.balance().amount().value() == 0 {
                                account.next_redeem_stake_batch = None;
                            } else {
                                account.next_redeem_stake_batch = Some(batch);
                            }
                            claimed_funds = true;
                        }
                    }
                }
            }
            None => {
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
            }
            Some(_) => {
                // this should never be reachable
                // while unstaking STAKE balances need to be locked, which means no receipts should be claimed
                return false;
            }
        }

        // shift the next batch into the current batch if the funds have been claimed for the current batch
        // and if the contract is not locked because it is running redeem stake batch workflow.
        //
        // NOTE: while a contrack is locked, all redeem requests must be collected in the next batch
        if self.run_redeem_stake_batch_lock.is_none() && account.redeem_stake_batch.is_none() {
            account.redeem_stake_batch = account.next_redeem_stake_batch.take();
        }

        claimed_funds
    }

    pub(crate) fn is_unstaking(&self) -> bool {
        match self.run_redeem_stake_batch_lock {
            Some(RedeemLock::Unstaking) => true,
            _ => false,
        }
    }

    pub fn stake_token_value(
        &self,
        total_staked_near_balance: domain::YoctoNear,
    ) -> domain::StakeTokenValue {
        domain::StakeTokenValue::new(
            domain::BlockTimeHeight::from_env(),
            total_staked_near_balance,
            self.total_stake.amount(),
        )
    }
}

type Balance = near_sdk::json_types::U128;

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolAccount {
    pub account_id: AccountId,
    /// The unstaked balance that can be withdrawn or staked.
    pub unstaked_balance: Balance,
    /// The amount balance staked at the current "stake" share price.
    pub staked_balance: Balance,
    /// Whether the unstaked balance is available for withdrawal now.
    pub can_withdraw: bool,
}

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn get_account(&self, account_id: AccountId) -> StakingPoolAccount;

    fn deposit(&mut self);

    fn deposit_and_stake(&mut self);

    fn stake(&mut self, amount: near_sdk::json_types::U128);

    fn unstake_all(&mut self);

    fn unstake(&mut self, amount: near_sdk::json_types::U128);

    fn get_account_staked_balance(&self, account_id: AccountId) -> near_sdk::json_types::U128;

    fn get_account_unstaked_balance(&self, account_id: AccountId) -> near_sdk::json_types::U128;

    fn is_account_unstaked_balance_available(&self, account_id: AccountId) -> bool;

    fn withdraw_all(&mut self);
}

#[ext_contract(ext_redeeming_workflow_callbacks)]
pub trait ExtRedeemingWokflowCallbacks {
    fn on_run_redeem_stake_batch(
        &mut self,
        #[callback] staked_balance: near_sdk::json_types::U128,
    ) -> Promise;

    /// ## Success Workflow
    /// 1. store the redeem stake batch receipt
    /// 2. set the redeem stake batch lock state to pending withdrawal
    fn on_unstake(&mut self);

    fn release_run_redeem_stake_batch_unstaking_lock(&mut self);

    /// batch ID is returned when all unstaked NEAR has been withdrawn
    fn on_redeeming_stake_pending_withdrawal(
        &mut self,
        #[callback] staking_pool_account: StakingPoolAccount,
    ) -> near_sdk::PromiseOrValue<BatchId>;

    fn on_redeeming_stake_post_withdrawal(&mut self) -> BatchId;
}

#[ext_contract(ext_staking_workflow_callbacks)]
pub trait ExtStakingWokflowCallbacks {
    /// callback for getting staked balance from staking pool as part of stake batch processing workflow
    ///
    /// ## Success Workflow
    /// 1. update the stake token value
    /// 2. deposit and stake funds with staking pool
    /// 3. register [on_deposit_and_stake] callback on the deposit and stake action
    fn on_run_stake_batch(
        &mut self,
        #[callback] staking_pool_account: StakingPoolAccount,
    ) -> Promise;

    /// ## Success Workflow
    /// 1. store the stake batch receipt
    /// 2. update the STAKE token supply with the new STAKE tokens that were issued
    fn on_deposit_and_stake(&mut self);

    /// defined on [Operator] interface
    fn release_run_stake_batch_lock(&mut self);
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{
        core::Hash,
        interface::{AccountManagement, Operator},
        near::{UNSTAKED_NEAR_FUNDS_NUM_EPOCHS_TO_UNLOCK, YOCTO},
        test_utils::*,
    };
    use near_sdk::{json_types::ValidAccountId, testing_env, MockedBlockchain};
    use std::convert::{TryFrom, TryInto};

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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();

        // Then the funds are deposited into the current stake batch on the account
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(stake_batch.balance.amount.value(), context.attached_deposit);
        assert_eq!(stake_batch.id, batch_id);
        assert!(account.next_stake_batch.is_none());

        // And the funds are deposited into the current stake batch on the contract
        assert_eq!(
            contract.stake_batch.unwrap().balance().amount(),
            context.attached_deposit.into()
        );
        assert!(contract.next_stake_batch.is_none());

        // add another deposit to the batch
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());
        let batch_id_2 = contract.deposit();
        assert_eq!(batch_id, batch_id_2);

        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(
            stake_batch.balance.amount.value(),
            context.attached_deposit * 2
        );
        assert_eq!(stake_batch.id, batch_id);
        assert!(account.next_stake_batch.is_none());

        // And the funds are deposited into the current stake batch on the contract
        assert_eq!(
            contract.stake_batch.unwrap().balance().amount().value(),
            context.attached_deposit * 2
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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();
        contract.run_stake_batch_locked = true;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        let stake_batch = account.next_stake_batch.unwrap();
        assert_eq!(stake_batch.balance.amount.value(), context.attached_deposit);
        assert_eq!(stake_batch.id, batch_id);
        assert!(account.stake_batch.is_none());

        // And the funds are deposited into the next stake batch on the contract
        assert_eq!(
            contract.next_stake_batch.unwrap().balance().amount(),
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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert!(account.next_stake_batch.is_none());
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(stake_batch.balance.amount.value(), context.attached_deposit);
        assert_eq!(stake_batch.id, batch_id);

        assert!(contract.next_stake_batch.is_none());
        assert_eq!(
            contract.stake_batch.unwrap().balance().amount(),
            context.attached_deposit.into()
        );

        contract.run_stake_batch_locked = true;

        context.attached_deposit = 50 * YOCTO;
        testing_env!(context.clone());

        let next_batch_id = contract.deposit();
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(account.stake_batch.unwrap().id, batch_id);
        let next_stake_batch = account.next_stake_batch.unwrap();
        assert_eq!(
            next_stake_batch.balance.amount.value(),
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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // should have no effect because there are no batches and no receipts
        let (mut account, _account_id_hash) = contract.registered_account(account_id);
        contract.claim_receipt_funds(&mut account);
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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.deposit_near_for_account_to_stake(&mut account, YOCTO.into());
        contract.save_account(&account_hash, &account);

        // When batch receipts are claimed
        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_hash, &account);
        // Then there should be no effect on the account
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(stake_batch.id, batch_id);
        assert_eq!(stake_batch.balance.amount, YOCTO.into());
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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.deposit_near_for_account_to_stake(&mut account, YOCTO.into());
        contract.save_account(&account_hash, &account);

        // Given there is a receipt for the batch
        // And the receipt exists for the stake batch
        // And STAKE token value = 1 NEAR
        let stake_token_value =
            domain::StakeTokenValue::new(Default::default(), YOCTO.into(), YOCTO.into());
        let receipt = domain::StakeBatchReceipt::new((2 * YOCTO).into(), stake_token_value);
        let batch_id = domain::BatchId(batch_id.into());
        contract.stake_batch_receipts.insert(&batch_id, &receipt);
        // When batch receipts are claimed
        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_hash, &account);
        // Assert
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(
            account.stake.unwrap().amount.0 .0,
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
        let batch_id = contract.deposit_near_for_account_to_stake(&mut account, YOCTO.into());
        contract.save_account(&account_hash, &account);
        // When batch receipts are claimed
        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_hash, &account);
        // Assert
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(
            account.stake.unwrap().amount.0 .0,
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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.deposit_near_for_account_to_stake(&mut account, (2 * YOCTO).into());
        contract.save_account(&account_hash, &account);

        // Given there is a receipt for the batch
        // And the receipt exists for the stake batch
        // And STAKE token value = 1 NEAR
        let stake_token_value =
            domain::StakeTokenValue::new(Default::default(), YOCTO.into(), YOCTO.into());
        let receipt = domain::StakeBatchReceipt::new((2 * YOCTO).into(), stake_token_value);
        let batch_id = domain::BatchId(batch_id.into());
        contract.stake_batch_receipts.insert(&batch_id, &receipt);
        // When batch receipts are claimed
        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_hash, &account);

        // Assert
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(
            account.stake.unwrap().amount.0 .0,
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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let stake_batch_id = domain::BatchId(
            contract
                .deposit_near_for_account_to_stake(&mut account, (2 * YOCTO).into())
                .into(),
        );
        assert_eq!(
            contract.stake_batch.unwrap().balance().amount(),
            (2 * YOCTO).into()
        );
        // locking the contract should deposit the funds into the next stake batch
        contract.run_stake_batch_locked = true;
        let next_stake_batch_id =
            contract.deposit_near_for_account_to_stake(&mut account, (3 * YOCTO).into());
        assert_eq!(
            contract.next_stake_batch.unwrap().balance().amount(),
            (3 * YOCTO).into()
        );
        contract.save_account(&account_hash, &account);

        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert_eq!(
            account.stake_batch.unwrap().balance.amount.value(),
            2 * YOCTO
        );
        assert_eq!(
            account.next_stake_batch.unwrap().balance.amount.value(),
            3 * YOCTO
        );

        contract.run_stake_batch_locked = false;

        // Given that the batches have receipts
        // And STAKE token value = 1 NEAR
        let stake_token_value =
            domain::StakeTokenValue::new(Default::default(), YOCTO.into(), YOCTO.into());
        let receipt = domain::StakeBatchReceipt::new((2 * YOCTO).into(), stake_token_value);
        contract
            .stake_batch_receipts
            .insert(&domain::BatchId(stake_batch_id.into()), &receipt);
        let receipt = domain::StakeBatchReceipt::new((3 * YOCTO).into(), stake_token_value);
        contract
            .stake_batch_receipts
            .insert(&domain::BatchId(next_stake_batch_id.into()), &receipt);
        // When batch receipts are claimed
        let (mut account, account_hash) = contract.registered_account(account_id);
        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_hash, &account);
        // then receipts should be deleted because all funds have been claimed
        assert!(contract
            .stake_batch_receipts
            .get(&domain::BatchId(stake_batch_id.into()))
            .is_none());

        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        // and the account batches have been cleared
        assert!(account.stake_batch.is_none());
        assert!(account.next_stake_batch.is_none());
        // and the STAKE tokens were claimed and credited to the account
        assert_eq!(account.stake.unwrap().amount.0 .0, 5 * YOCTO);
    }

    /// Given there is no stake batch to run
    /// Then the call fails
    #[test]
    #[should_panic(expected = "there is no stake batch to run")]
    fn stake_no_stake_batch() {
        let account_id = "alfio-zappala.near";
        let context = new_context(account_id);
        testing_env!(context);

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.stake();
    }

    /// Given the contract has a stake batch
    /// When the stake batch is run
    /// Then the contract is locked
    /// When the stake batch is run again while the contract is locked
    /// Then the func call panics
    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn stake_contract_when_stake_batch_in_progress() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        context.attached_deposit = YOCTO;
        contract.deposit();

        context.attached_deposit = 0;
        testing_env!(context.clone());
        contract.stake();
        assert!(contract.run_stake_batch_locked);

        testing_env!(context.clone());
        // should panic because contract is locked
        contract.stake();
    }

    #[test]
    fn deposit_and_stake_contract_when_stake_batch_in_progress() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        if let PromiseOrValue::Promise(_) = contract.deposit_and_stake() {
            if let PromiseOrValue::Value(batch_id) = contract.deposit_and_stake() {
                assert_eq!(batch_id, contract.next_stake_batch.unwrap().id().into());
            } else {
                panic!("expected staking batch to be in progress");
            }
        } else {
            panic!("expected deposit to be staked");
        }
    }

    /// Given the contract is running the redeem stake batch
    /// When the stake batch is run
    /// Then the func call panics
    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn stake_contract_when_redeem_stake_batch_in_progress_unstaking() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        contract.register_account();
        contract.stake();
    }

    #[test]
    fn deposit_and_stake_contract_when_redeem_stake_batch_in_progress_unstaking() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        contract.register_account();
        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        if let PromiseOrValue::Value(batch_id) = contract.deposit_and_stake() {
            assert_eq!(batch_id, contract.stake_batch.unwrap().id().into());
        } else {
            panic!("expected staking batch to be in progress");
        }
    }

    /// Given the contract is redeem status is pending withdrawal
    /// Then it is allowed to run stake batches
    #[test]
    fn stake_contract_when_redeem_status_pending_withdrawal() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.stake();
    }

    #[test]
    fn deposit_and_stake_contract_when_redeem_status_pending_withdrawal() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        *contract.batch_id_sequence += 1;
        let redeem_stake_batch =
            domain::RedeemStakeBatch::new(contract.batch_id_sequence, YOCTO.into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);

        contract.register_account();

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.deposit_and_stake();
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
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.total_stake.credit(YOCTO.into());
        contract.stake_token_value =
            domain::StakeTokenValue::new(Default::default(), YOCTO.into(), YOCTO.into());

        assert_eq!(
            contract.stake_token_value.total_stake_supply(),
            contract.total_stake.amount().into()
        );
        assert_eq!(
            contract.stake_token_value.total_staked_near_balance(),
            YOCTO.into()
        );
    }

    /// Given the contract has a stake batch
    /// And the contract is not locked
    /// When the stake batch is run
    /// Then it generates to FunctionCall receipts:
    ///   1. to get the staked balance from the staking pool contract
    ///   2. and then to callback into this contract - on_run_stake_batch
    ///   3. and finally a callback into this contract to unlock the contract
    #[test]
    fn stake_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();

        context.attached_deposit = YOCTO;
        contract.deposit();

        context.prepaid_gas = 10u64.pow(18);
        testing_env!(context.clone());
        contract.stake();
        assert!(contract.run_stake_batch_locked);
        println!(
            "prepaid gas: {}, used_gas: {}, unused_gas: {}",
            context.prepaid_gas,
            env::used_gas(),
            context.prepaid_gas - env::used_gas()
        );

        let receipts: Vec<Receipt> = deserialize_receipts(&env::created_receipts());
        assert_eq!(receipts.len(), 3);

        // there should be a `get_account_staked_balance` func call on the staking pool
        let _get_staked_balance_func_call = receipts
            .iter()
            .find(|receipt| {
                if receipt.receiver_id.as_str()
                    == contract_settings.staking_pool_id.as_ref().as_str()
                {
                    if let Some(Action::FunctionCall { method_name, .. }) = receipt.actions.first()
                    {
                        method_name == "get_account"
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap();

        // and a callback - `on_run_stake_batch`
        let on_run_stake_batch_func_call = receipts
            .iter()
            .find(|receipt| {
                if receipt.receiver_id == context.current_account_id {
                    if let Some(Action::FunctionCall { method_name, .. }) = receipt.actions.first()
                    {
                        method_name == "on_run_stake_batch"
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap();
        // and the callback requires a data receipt, i.e., the staked balance
        assert_eq!(on_run_stake_batch_func_call.receipt_indices.len(), 1);
        assert_eq!(
            *on_run_stake_batch_func_call
                .receipt_indices
                .first()
                .unwrap(),
            0
        );

        // and a callback - `unlock`
        let _unlock = receipts
            .iter()
            .find(|receipt| {
                if receipt.receiver_id == context.current_account_id {
                    if let Some(Action::FunctionCall { method_name, .. }) = receipt.actions.first()
                    {
                        method_name == "release_run_stake_batch_lock"
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap();
    }

    #[test]
    fn deposit_and_stake_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.deposit_and_stake();

        assert!(contract.run_stake_batch_locked);
        println!(
            "prepaid gas: {}, used_gas: {}, unused_gas: {}",
            context.prepaid_gas,
            env::used_gas(),
            context.prepaid_gas - env::used_gas()
        );

        let receipts: Vec<Receipt> = deserialize_receipts(&env::created_receipts());
        assert_eq!(receipts.len(), 3);

        // there should be a `get_account_staked_balance` func call on the staking pool
        let _get_staked_balance_func_call = receipts
            .iter()
            .find(|receipt| {
                if receipt.receiver_id.as_str()
                    == contract_settings.staking_pool_id.as_ref().as_str()
                {
                    if let Some(Action::FunctionCall { method_name, .. }) = receipt.actions.first()
                    {
                        method_name == "get_account"
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap();

        // and a callback - `on_run_stake_batch`
        let on_run_stake_batch_func_call = receipts
            .iter()
            .find(|receipt| {
                if receipt.receiver_id == context.current_account_id {
                    if let Some(Action::FunctionCall { method_name, .. }) = receipt.actions.first()
                    {
                        method_name == "on_run_stake_batch"
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap();
        // and the callback requires a data receipt, i.e., the staked balance
        assert_eq!(on_run_stake_batch_func_call.receipt_indices.len(), 1);
        assert_eq!(
            *on_run_stake_batch_func_call
                .receipt_indices
                .first()
                .unwrap(),
            0
        );

        // and a callback - `unlock`
        let _unlock = receipts
            .iter()
            .find(|receipt| {
                if receipt.receiver_id == context.current_account_id {
                    if let Some(Action::FunctionCall { method_name, .. }) = receipt.actions.first()
                    {
                        method_name == "release_run_stake_batch_lock"
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap();
    }

    /// Given the funds were successfully deposited and staked into the staking pool
    /// Then the stake batch receipts is saved
    /// And the total STAKE supply is updated
    /// And if there are funds in the next stake batch, then move it into the current batch
    #[test]
    fn stake_workflow_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

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
                contract.stake();
                assert!(contract.run_stake_batch_locked);
                {
                    context.predecessor_account_id = context.current_account_id.clone();
                    testing_env!(context.clone());
                    let staking_pool_account = StakingPoolAccount {
                        account_id: context.predecessor_account_id,
                        unstaked_balance: 0.into(),
                        staked_balance: 0.into(),
                        can_withdraw: true,
                    };
                    contract.on_run_stake_batch(staking_pool_account); // callback

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

    /// Given a registered account has STAKE
    /// And there are no contract locks, i.e., no batches are being run
    /// When the account redeems STAKE
    /// Then the STAKE funds are moved from the the account's STAKE balance to the account's current redeem stake batch
    /// And the contract redeem stake batch is credited
    /// When the account redeems more STAKE
    /// And the batch has not yet run
    /// Then the STAKE will be added to the batch
    #[test]
    fn redeem_no_locks() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();
        assert!(contract.redeem_stake_batch.is_none());
        assert!(contract.next_redeem_stake_batch.is_none());

        // Given the account has STAKE
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        let initial_account_stake = (50 * YOCTO).into();
        account.apply_stake_credit(initial_account_stake);
        contract.save_account(&account_hash, &account);

        let redeem_amount = YoctoStake::from(10 * YOCTO);
        let batch_id = contract.redeem(redeem_amount.clone());

        let batch = contract
            .redeem_stake_batch
            .expect("current stake batch should have funds");
        assert_eq!(batch_id, batch.id().into());
        assert_eq!(redeem_amount, batch.balance().amount().into());

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        // assert STAKE was moved from account STAKE balance to redeem stake batch
        assert_eq!(
            account.stake.unwrap().amount,
            (initial_account_stake.value() - redeem_amount.value()).into()
        );
        let redeem_stake_batch = account.redeem_stake_batch.unwrap();
        assert_eq!(redeem_stake_batch.balance.amount, redeem_amount);
        assert_eq!(redeem_stake_batch.id, batch_id);

        let _batch_id_2 = contract.redeem(redeem_amount.clone());

        let batch = contract
            .redeem_stake_batch
            .expect("current stake batch should have funds");
        assert_eq!(batch_id, batch.id().into());
        assert_eq!(redeem_amount.value() * 2, batch.balance().amount().value());

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        // assert STAKE was moved from account STAKE balance to redeem stake batch
        assert_eq!(
            account.stake.unwrap().amount,
            (initial_account_stake.value() - (redeem_amount.value() * 2)).into()
        );
        let redeem_stake_batch = account.redeem_stake_batch.unwrap();
        assert_eq!(
            redeem_stake_batch.balance.amount,
            (redeem_amount.value() * 2).into()
        );
        assert_eq!(redeem_stake_batch.id, batch_id);
    }

    /// Given a registered account has STAKE
    /// And there are no contract locks, i.e., no batches are being run
    /// When the account redeems STAKE
    /// Then the STAKE funds are moved from the the account's STAKE balance to the account's current redeem stake batch
    /// And the contract redeem stake batch is credited
    /// Given the contract is locked on the redeem stake batch for unstaking
    /// When the account redeems more STAKE
    /// Then the STAKE will be added to the next batch
    #[test]
    fn redeem_while_redeem_stake_batch_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();
        assert!(contract.redeem_stake_batch.is_none());
        assert!(contract.next_redeem_stake_batch.is_none());

        // Given the account has STAKE
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        let initial_account_stake = (50 * YOCTO).into();
        account.apply_stake_credit(initial_account_stake);
        contract.save_account(&account_hash, &account);

        let redeem_amount = YoctoStake::from(10 * YOCTO);
        let batch_id = contract.redeem(redeem_amount.clone());

        let batch = contract
            .redeem_stake_batch
            .expect("current stake batch should have funds");
        assert_eq!(batch_id, batch.id().into());
        assert_eq!(redeem_amount, batch.balance().amount().into());

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        // assert STAKE was moved from account STAKE balance to redeem stake batch
        assert_eq!(
            account.stake.unwrap().amount,
            (initial_account_stake.value() - redeem_amount.value()).into()
        );
        let redeem_stake_batch = account.redeem_stake_batch.unwrap();
        assert_eq!(redeem_stake_batch.balance.amount, redeem_amount);
        assert_eq!(redeem_stake_batch.id, batch_id);

        // Given the contract is locked for unstaking
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);
        let batch_id_2 = contract.redeem(redeem_amount.clone());

        let batch = contract
            .redeem_stake_batch
            .expect("current stake batch should have funds");
        assert_eq!(redeem_amount.value(), batch.balance().amount().value());

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(
            account.stake.unwrap().amount,
            (initial_account_stake.value() - (redeem_amount.value() * 2)).into()
        );
        let redeem_stake_batch = account.redeem_stake_batch.unwrap();
        assert_eq!(
            redeem_stake_batch.balance.amount,
            (redeem_amount.value()).into()
        );
        assert_eq!(redeem_stake_batch.id, batch_id);

        let next_redeem_stake_batch = account.next_redeem_stake_batch.unwrap();
        assert_eq!(
            next_redeem_stake_batch.balance.amount,
            (redeem_amount.value()).into()
        );
        assert_eq!(next_redeem_stake_batch.id, batch_id_2);
    }

    /// Given an account has unclaimed stake batch receipts
    /// When the account tries to redeem STAKE
    /// Then the stake batch receipts are first claimed before checking the account balance
    #[test]
    fn redeem_with_unclaimed_stake_batch_receipts() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();
        context.attached_deposit = 5 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        // Given an account has unclaimed stake batch receipts
        let batch = contract.stake_batch.unwrap();
        let receipt =
            domain::StakeBatchReceipt::new(batch.balance().amount(), contract.stake_token_value);
        contract.stake_batch_receipts.insert(&batch.id(), &receipt);

        // When the account tries to redeem STAKE
        testing_env!(context.clone());
        contract.redeem((2 * YOCTO).into());

        let (account, _account_hash_id) = contract.registered_account(account_id);
        assert_eq!(account.stake.unwrap().amount(), (3 * YOCTO).into());
        assert_eq!(
            account.redeem_stake_batch.unwrap().balance().amount(),
            (2 * YOCTO).into()
        );
    }

    /// Given an account has unclaimed stake batch receipts
    /// When the account tries to redeem STAKE
    /// Then the stake batch receipts are first claimed before checking the account balance
    #[test]
    fn redeem_all_with_unclaimed_stake_batch_receipts() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();
        context.attached_deposit = 5 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        // Given an account has unclaimed stake batch receipts
        let batch = contract.stake_batch.unwrap();
        let receipt =
            domain::StakeBatchReceipt::new(batch.balance().amount(), contract.stake_token_value);
        contract.stake_batch_receipts.insert(&batch.id(), &receipt);

        // When the account tries to redeem STAKE
        testing_env!(context.clone());
        contract.redeem_all();

        let (account, _account_hash_id) = contract.registered_account(account_id);
        assert!(account.stake.is_none());
        assert_eq!(
            account.redeem_stake_batch.unwrap().balance().amount(),
            batch.balance().amount().value().into()
        );
    }

    /// Given a registered account has STAKE
    /// And there are no contract locks, i.e., no batches are being run
    /// When the account redeems all STAKE
    /// Then the STAKE funds are moved from the the account's STAKE balance to the account's current redeem stake batch
    /// And the contract redeem stake batch is credited
    #[test]
    fn redeem_all_with_redeem_lock_unstaking() {
        redeem_all_with_lock(RedeemLock::Unstaking);
    }

    #[test]
    fn redeem_all_with_redeem_lock_pending_withdrawal() {
        redeem_all_with_lock(RedeemLock::PendingWithdrawal);
    }

    fn redeem_all_with_lock(lock: RedeemLock) {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        contract.register_account();
        assert!(contract.redeem_stake_batch.is_none());
        assert!(contract.next_redeem_stake_batch.is_none());

        // Given the account has STAKE
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        let initial_account_stake = (50 * YOCTO).into();
        account.apply_stake_credit(initial_account_stake);
        contract.save_account(&account_hash, &account);

        let batch_id = contract.redeem_all();
        contract.run_redeem_stake_batch_lock = Some(lock);

        let batch = contract
            .redeem_stake_batch
            .expect("next stake batch should have funds");
        assert_eq!(batch_id, batch.id().into());
        assert_eq!(
            initial_account_stake.value(),
            batch.balance().amount().value()
        );

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        // assert STAKE was moved from account STAKE balance to redeem stake batch
        assert!(account.stake.is_none());
        let redeem_stake_batch = account
            .redeem_stake_batch
            .expect("redeemed STAKE should have been put into batch");
        assert_eq!(
            redeem_stake_batch.balance.amount,
            initial_account_stake.into()
        );
        assert_eq!(redeem_stake_batch.id, batch_id);
    }

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct GetStakedAccountBalanceArgs {
        account_id: String,
    }

    /// Given the contract is unlocked and has no batch runs in progress
    /// And there is a redeem stake batch
    /// When the redeem batch is run
    /// Then it creates the following receipts
    ///   - func call to get account from staking pool
    ///   - func call for callback to clear the release lock if the state is `Unstaking`
    #[test]
    fn unstake_no_locks() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        *contract.batch_id_sequence += 1;
        contract.redeem_stake_batch = Some(RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));

        contract.unstake();
        assert_eq!(
            contract.run_redeem_stake_batch_lock,
            Some(RedeemLock::Unstaking)
        );
        let receipts = deserialize_receipts(&env::created_receipts());
        println!("receipt count = {}\n{:#?}", receipts.len(), receipts);
        assert_eq!(receipts.len(), 3);
        let receipts = receipts.as_slice();
        {
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, contract.staking_pool_id);

            let actions = receipt.actions.as_slice();
            let func_call_action = actions.first().unwrap();
            match func_call_action {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "get_account");
                    let args: GetStakedAccountBalanceArgs =
                        near_sdk::serde_json::from_str(args).unwrap();
                    assert_eq!(args.account_id, context.current_account_id);
                }
                _ => panic!("expected func call action"),
            }
        }
        {
            let receipt = &receipts[1];
            assert_eq!(receipt.receiver_id, env::current_account_id());

            let actions = receipt.actions.as_slice();
            let func_call_action = actions.first().unwrap();
            match func_call_action {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "on_run_redeem_stake_batch");
                    assert!(args.is_empty());
                }
                _ => panic!("expected func call action"),
            }
        }
        {
            let receipt = &receipts[2];
            assert_eq!(receipt.receiver_id, env::current_account_id());

            let actions = receipt.actions.as_slice();
            let func_call_action = actions.first().unwrap();
            match func_call_action {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "release_run_redeem_stake_batch_unstaking_lock");
                    assert!(args.is_empty());
                }
                _ => panic!("expected func call action"),
            }
        }
    }

    #[test]
    fn redeem_and_unstake_no_locks() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        testing_env!(context.clone());
        contract.register_account();
        let (mut account, account_id_hash) = contract.registered_account(account_id);
        account.stake = Some(TimestampedStakeBalance::new((100 * YOCTO).into()));
        contract.accounts.insert(&account_id_hash, &account);

        testing_env!(context.clone());
        contract.redeem_and_unstake((10 * YOCTO).into());

        assert_eq!(
            contract.run_redeem_stake_batch_lock,
            Some(RedeemLock::Unstaking)
        );
        let receipts = deserialize_receipts(&env::created_receipts());
        println!("receipt count = {}\n{:#?}", receipts.len(), receipts);
        assert_eq!(receipts.len(), 3);
        let receipts = receipts.as_slice();
        {
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, contract.staking_pool_id);

            let actions = receipt.actions.as_slice();
            let func_call_action = actions.first().unwrap();
            match func_call_action {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "get_account");
                    let args: GetStakedAccountBalanceArgs =
                        near_sdk::serde_json::from_str(args).unwrap();
                    assert_eq!(args.account_id, context.current_account_id);
                }
                _ => panic!("expected func call action"),
            }
        }
        {
            let receipt = &receipts[1];
            assert_eq!(receipt.receiver_id, env::current_account_id());

            let actions = receipt.actions.as_slice();
            let func_call_action = actions.first().unwrap();
            match func_call_action {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "on_run_redeem_stake_batch");
                    assert!(args.is_empty());
                }
                _ => panic!("expected func call action"),
            }
        }
        {
            let receipt = &receipts[2];
            assert_eq!(receipt.receiver_id, env::current_account_id());

            let actions = receipt.actions.as_slice();
            let func_call_action = actions.first().unwrap();
            match func_call_action {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "release_run_redeem_stake_batch_unstaking_lock");
                    assert!(args.is_empty());
                }
                _ => panic!("expected func call action"),
            }
        }
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn unstake_locked_for_staking() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());
        contract.run_stake_batch_locked = true;
        contract.unstake();
    }

    #[test]
    fn redeem_and_unstake_locked_for_staking() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());
        contract.run_stake_batch_locked = true;

        contract.register_account();
        let (mut account, account_id_hash) = contract.registered_account(account_id);
        account.stake = Some(TimestampedStakeBalance::new((100 * YOCTO).into()));
        contract.accounts.insert(&account_id_hash, &account);

        testing_env!(context.clone());
        if let PromiseOrValue::Value(batch_id) = contract.redeem_and_unstake((10 * YOCTO).into()) {
            assert_eq!(batch_id, contract.redeem_stake_batch.unwrap().id().into());
        } else {
            panic!("expected batch ID to be returned because unstake workflow cannot be run if a batch is running");
        }
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn unstake_locked_for_unstaking() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);
        contract.unstake();
    }

    #[test]
    fn redeem_and_unstake_locked_for_unstaking() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        contract.register_account();
        let (mut account, account_id_hash) = contract.registered_account(account_id);
        account.stake = Some(TimestampedStakeBalance::new((100 * YOCTO).into()));
        contract.accounts.insert(&account_id_hash, &account);

        testing_env!(context.clone());
        if let PromiseOrValue::Value(batch_id) = contract.redeem_and_unstake((10 * YOCTO).into()) {
            assert_eq!(
                batch_id,
                contract.next_redeem_stake_batch.unwrap().id().into()
            );
        } else {
            panic!("expected batch ID to be returned because unstake workflow cannot be run if a batch is running");
        }
    }

    #[test]
    #[should_panic(expected = "there is no redeem stake batch")]
    fn unstake_no_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.unstake();
    }

    /// Given the contract is unlocked and has no batch runs in progress
    /// And there is a redeem stake batch
    /// When the redeem batch is run
    /// Then it creates the following receipts
    ///   - func call to get account from staking pool
    ///   - func call for callback to clear the release lock if the state is `Unstaking`
    #[test]
    fn unstake_pending_withdrawal() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        *contract.batch_id_sequence += 1;
        contract.redeem_stake_batch = Some(RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new((10 * YOCTO).into(), contract.stake_token_value),
        );
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        context.epoch_height += UNSTAKED_NEAR_FUNDS_NUM_EPOCHS_TO_UNLOCK.value();
        testing_env!(context.clone());
        contract.unstake();
        assert_eq!(
            contract.run_redeem_stake_batch_lock,
            Some(RedeemLock::PendingWithdrawal)
        );
        let receipts = deserialize_receipts(&env::created_receipts());
        println!("receipt count = {}\n{:#?}", receipts.len(), receipts);
        assert_eq!(receipts.len(), 2);
        let receipts = receipts.as_slice();
        {
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, contract.staking_pool_id);

            let actions = receipt.actions.as_slice();
            let func_call_action = actions.first().unwrap();
            match func_call_action {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "get_account");
                    assert_eq!(args, "{\"account_id\":\"stake.oysterpack.near\"}");
                }
                _ => panic!("expected func call action"),
            }
        }
        {
            let receipt = &receipts[1];
            assert_eq!(receipt.receiver_id, env::current_account_id());

            let actions = receipt.actions.as_slice();
            let func_call_action = actions.first().unwrap();
            match func_call_action {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "on_redeeming_stake_pending_withdrawal");
                    assert!(args.is_empty());
                }
                _ => panic!("expected func call action"),
            }
        }
    }

    //     Some(RedeemLock::PendingWithdrawal) => {
    // let batch = self
    // .redeem_stake_batch
    // .expect("illegal state - batch does not exist");
    // let batch_id = batch.id();
    // let batch_receipt = self
    // .redeem_stake_batch_receipts
    // .get(&batch_id)
    // .expect("illegal state - batch receipt does not exist");

    #[test]
    #[should_panic(expected = "ILLEGAL STATE : redeem stake batch should exist")]
    fn unstake_pending_withdrawal_with_batch_not_exists() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.unstake();
    }

    #[test]
    #[should_panic(expected = "ILLEGAL STATE : redeem stake batch receipt should exist")]
    fn unstake_pending_withdrawal_with_batch_receipt_not_exists() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        *contract.batch_id_sequence += 1;
        contract.redeem_stake_batch = Some(RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.unstake();
    }

    #[test]
    #[should_panic(expected = "unstaked funds are not yet available for withdrawal")]
    fn unstake_pending_withdrawal_cannot_withdraw() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        *contract.batch_id_sequence += 1;
        contract.redeem_stake_batch = Some(RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new((10 * YOCTO).into(), contract.stake_token_value),
        );
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.unstake();
    }

    /// Given an account has redeemed STAKE
    /// And the batch has completed
    /// Then the account can claim the NEAR funds
    #[test]
    fn claim_redeem_stake_batch_receipts_for_current_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();

        let account_id_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_id_hash).unwrap();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.save_account(&account_id_hash, &account);

        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new((20 * YOCTO).into(), contract.stake_token_value),
        );

        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_id_hash, &account);
        let account = contract.accounts.get(&account_id_hash).unwrap();
        assert_eq!(account.near.unwrap().amount(), (10 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());

        // Then there should be 10 STAKE left unclaimed on the receipt
        let receipt = contract
            .redeem_stake_batch_receipts
            .get(&contract.batch_id_sequence)
            .unwrap();
        assert_eq!(receipt.redeemed_stake(), (10 * YOCTO).into());
    }

    #[test]
    fn claim_redeem_stake_batch_receipts_for_current_and_next_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();

        let account_id_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_id_hash).unwrap();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        *contract.batch_id_sequence += 1;
        account.next_redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (15 * YOCTO).into(),
        ));
        contract.save_account(&account_id_hash, &account);

        contract.redeem_stake_batch_receipts.insert(
            &(contract.batch_id_sequence.value() - 1).into(),
            &domain::RedeemStakeBatchReceipt::new((10 * YOCTO).into(), contract.stake_token_value),
        );
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new((20 * YOCTO).into(), contract.stake_token_value),
        );

        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_id_hash, &account);
        let account = contract.accounts.get(&account_id_hash).unwrap();
        assert_eq!(account.near.unwrap().amount(), (25 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        assert!(contract
            .redeem_stake_batch_receipts
            .get(&(contract.batch_id_sequence.value() - 1).into())
            .is_none());
        assert_eq!(
            contract
                .redeem_stake_batch_receipts
                .get(&contract.batch_id_sequence)
                .unwrap()
                .redeemed_stake(),
            (5 * YOCTO).into()
        );
    }

    /// Given an account has redeemed STAKE
    /// And the batch receipt is pending withdrawal
    /// And there is enough NEAR liquidity to fulfill the claim
    /// Then the account can claim the NEAR funds from the NEAR liquidity pool
    #[test]
    fn claim_redeem_stake_batch_receipts_for_current_batch_pending_withdrawal_with_full_near_liquidity_available(
    ) {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();

        let account_id_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_id_hash).unwrap();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.save_account(&account_id_hash, &account);

        contract.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (20 * YOCTO).into(),
        ));
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.near_liquidity_pool = contract
            .stake_token_value
            .stake_to_near(account.redeem_stake_batch.unwrap().balance().amount());
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new(
                contract.redeem_stake_batch.unwrap().balance().amount(),
                contract.stake_token_value,
            ),
        );

        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_id_hash, &account);
        let account = contract.accounts.get(&account_id_hash).unwrap();
        assert_eq!(account.near.unwrap().amount(), (10 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());

        // Then there should be 10 STAKE left unclaimed on the receipt
        let receipt = contract
            .redeem_stake_batch_receipts
            .get(&contract.batch_id_sequence)
            .unwrap();
        assert_eq!(receipt.redeemed_stake(), (10 * YOCTO).into());
        assert_eq!(contract.near_liquidity_pool, 0.into());
        assert_eq!(contract.total_near.amount(), (10 * YOCTO).into());
    }

    /// Given an account has redeemed STAKE
    /// And the batch receipt is pending withdrawal
    /// And there is enough NEAR liquidity to fulfill the claim
    /// And the receipt is fully claimed
    /// Then the account can claim the NEAR funds from the NEAR liquidity pool
    /// And the RedeemLock is set to None
    /// And the receipt has been deleted
    #[test]
    fn claim_redeem_stake_batch_receipts_for_current_batch_pending_withdrawal_with_full_near_liquidity_available_and_receipt_fully_claimed(
    ) {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();

        let account_id_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_id_hash).unwrap();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.save_account(&account_id_hash, &account);

        contract.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.near_liquidity_pool = contract
            .stake_token_value
            .stake_to_near(account.redeem_stake_batch.unwrap().balance().amount());
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new(
                contract.redeem_stake_batch.unwrap().balance().amount(),
                contract.stake_token_value,
            ),
        );

        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_id_hash, &account);
        let account = contract.accounts.get(&account_id_hash).unwrap();
        assert_eq!(account.near.unwrap().amount(), (10 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());

        // Then there should be 10 STAKE left unclaimed on the receipt
        assert!(contract
            .redeem_stake_batch_receipts
            .get(&contract.batch_id_sequence)
            .is_none());
        assert!(contract.run_redeem_stake_batch_lock.is_none());
        assert_eq!(contract.near_liquidity_pool, 0.into());
        assert_eq!(contract.total_near.amount(), (10 * YOCTO).into());
    }

    /// Given an account has redeemed STAKE into the current and next batches
    /// And there is a receipt for the current batch
    /// When the account claims funds, the current batch funds will be claimed
    /// And the next batch gets moved into the current batch slot
    #[test]
    fn claim_redeem_stake_batch_receipts_for_current_and_next_batch_with_receipt_for_current() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());

        contract.register_account();

        let account_id_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_id_hash).unwrap();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        *contract.batch_id_sequence += 1;
        account.next_redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (15 * YOCTO).into(),
        ));
        contract.save_account(&account_id_hash, &account);

        contract.redeem_stake_batch_receipts.insert(
            &(contract.batch_id_sequence.value() - 1).into(),
            &domain::RedeemStakeBatchReceipt::new((10 * YOCTO).into(), contract.stake_token_value),
        );

        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_id_hash, &account);
        let account = contract.accounts.get(&account_id_hash).unwrap();
        assert_eq!(account.near.unwrap().amount(), (10 * YOCTO).into());
        assert_eq!(
            account.redeem_stake_batch.unwrap().balance().amount(),
            (15 * YOCTO).into()
        );
        assert!(account.next_redeem_stake_batch.is_none());
        assert!(contract
            .redeem_stake_batch_receipts
            .get(&(contract.batch_id_sequence.value() - 1).into())
            .is_none());
    }

    /// Given an account has redeemed STAKE
    /// And the batch has completed
    /// And there is a current batch pending withdrawal
    /// Then the account can claim the NEAR funds
    #[test]
    fn claim_redeem_stake_batch_receipts_for_old_batch_receipt_while_pending_withdrawal_on_current_batch(
    ) {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings.clone());
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);

        contract.register_account();

        let account_id_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_id_hash).unwrap();
        let batch_id = contract.batch_id_sequence;
        account.redeem_stake_batch =
            Some(domain::RedeemStakeBatch::new(batch_id, (10 * YOCTO).into()));
        account.next_redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            (batch_id.value() + 1).into(),
            (10 * YOCTO).into(),
        ));
        contract.save_account(&account_id_hash, &account);

        *contract.batch_id_sequence += 10;
        contract.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (100 * YOCTO).into(),
        ));

        contract.redeem_stake_batch_receipts.insert(
            &batch_id,
            &domain::RedeemStakeBatchReceipt::new((20 * YOCTO).into(), contract.stake_token_value),
        );
        contract.redeem_stake_batch_receipts.insert(
            &(batch_id.value() + 1).into(),
            &domain::RedeemStakeBatchReceipt::new((20 * YOCTO).into(), contract.stake_token_value),
        );

        contract.claim_receipt_funds(&mut account);
        contract.save_account(&account_id_hash, &account);
        let account = contract.accounts.get(&account_id_hash).unwrap();
        assert_eq!(account.near.unwrap().amount(), (20 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());

        let receipt = contract.redeem_stake_batch_receipts.get(&batch_id).unwrap();
        assert_eq!(receipt.redeemed_stake(), (10 * YOCTO).into());
    }

    #[test]
    fn apply_unclaimed_receipts_to_account() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        let (mut account, account_id_hash) = contract.registered_account(account_id);

        {
            let batch = contract.stake_batch.unwrap();
            // create a stake batch receipt for the stake batch
            let receipt = domain::StakeBatchReceipt::new(
                batch.balance().amount(),
                contract.stake_token_value,
            );
            contract.stake_batch_receipts.insert(&batch.id(), &receipt);

            *contract.batch_id_sequence += 1;
            let batch = domain::StakeBatch::new(contract.batch_id_sequence, (10 * YOCTO).into());
            account.next_stake_batch = Some(batch);
            let receipt = domain::StakeBatchReceipt::new(
                batch.balance().amount(),
                contract.stake_token_value,
            );
            contract
                .stake_batch_receipts
                .insert(&contract.batch_id_sequence, &receipt);

            contract.stake_batch = None;
            contract.next_stake_batch = None;
        }

        {
            // create a redeem stake batch receipt for 2 yoctoSTAKE
            *contract.batch_id_sequence += 1;
            let redeem_stake_batch =
                domain::RedeemStakeBatch::new(contract.batch_id_sequence, (2 * YOCTO).into());
            contract.redeem_stake_batch_receipts.insert(
                &contract.batch_id_sequence,
                &domain::RedeemStakeBatchReceipt::new(
                    redeem_stake_batch.balance().amount(),
                    contract.stake_token_value,
                ),
            );
            account.redeem_stake_batch = Some(redeem_stake_batch);

            *contract.batch_id_sequence += 1;
            let redeem_stake_batch =
                domain::RedeemStakeBatch::new(contract.batch_id_sequence, (2 * YOCTO).into());
            contract.redeem_stake_batch_receipts.insert(
                &contract.batch_id_sequence,
                &domain::RedeemStakeBatchReceipt::new(
                    redeem_stake_batch.balance().amount(),
                    contract.stake_token_value,
                ),
            );
            account.next_redeem_stake_batch = Some(redeem_stake_batch);
        }
        contract.save_account(&account_id_hash, &account);

        context.is_view = true;
        testing_env!(context.clone());
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        assert_eq!(account.stake.unwrap().amount, (2 * 10 * YOCTO).into());
        assert_eq!(account.near.unwrap().amount, (2 * 2 * YOCTO).into());
    }

    /// Given an account has deposited funds into a stake batch
    /// And the contract is not locked
    /// When the account tries to withdraw funds from the batch
    /// Then the funds are transferred back to the account
    #[test]
    fn withdraw_funds_from_stake_batch_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_funds_from_stake_batch(YOCTO.into());

        {
            let receipts = deserialize_receipts(&env::created_receipts());
            println!("{:#?}", &receipts);
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(
            account.stake_batch.unwrap().balance.amount.value(),
            (9 * YOCTO)
        );
        assert_eq!(
            contract.stake_batch.unwrap().balance().amount().value(),
            (9 * YOCTO)
        );
    }

    #[test]
    fn withdraw_funds_from_stake_batch_all_funds_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_funds_from_stake_batch(context.attached_deposit.into());

        {
            let receipts = deserialize_receipts(&env::created_receipts());
            println!("{:#?}", &receipts);
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, context.attached_deposit),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
    }

    /// Given an account has deposited funds into the next stake batch
    /// And the contract is locked
    /// When the account tries to withdraw funds from the batch
    /// Then the funds are transferred back to the account
    #[test]
    fn withdraw_funds_from_stake_batch_while_stake_batch_run_locked_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();
        contract.run_stake_batch_locked = true;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_funds_from_stake_batch(YOCTO.into());

        {
            let receipts = deserialize_receipts(&env::created_receipts());
            println!("{:#?}", &receipts);
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(
            account.next_stake_batch.unwrap().balance.amount.value(),
            (9 * YOCTO)
        );
    }

    /// Given an account has deposited funds into the next stake batch
    /// And the contract is locked
    /// When the account tries to withdraw funds from the batch
    /// Then the funds are transferred back to the account
    #[test]
    fn withdraw_funds_from_stake_batch_while_stake_batch_run_locked_all_funds_auccess() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();
        contract.run_stake_batch_locked = true;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_funds_from_stake_batch(context.attached_deposit.into());

        {
            let receipts = deserialize_receipts(&env::created_receipts());
            println!("{:#?}", &receipts);
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, context.attached_deposit),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert!(account.next_stake_batch.is_none());
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn withdraw_funds_from_stake_batch_while_stake_batch_run_locked_and_stake_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_stake_batch_locked = true;

        testing_env!(context.clone());
        contract.withdraw_funds_from_stake_batch(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn withdraw_funds_from_stake_batch_while_unstaking_and_stake_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        testing_env!(context.clone());
        contract.withdraw_funds_from_stake_batch(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "there are no funds in stake batch")]
    fn withdraw_funds_from_stake_batch_no_stake_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        testing_env!(context.clone());
        contract.withdraw_funds_from_stake_batch(YOCTO.into());
    }

    /// Given an account has deposited funds into a stake batch
    /// And the contract is not locked
    /// When the account tries to withdraw funds from the batch
    /// Then the funds are transferred back to the account
    #[test]
    fn withdraw_all_funds_from_stake_batch_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_all_funds_from_stake_batch();

        {
            let receipts = deserialize_receipts(&env::created_receipts());
            println!("{:#?}", &receipts);
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, 10 * YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        assert!(contract.stake_batch.is_none());
    }

    /// Given an account has deposited funds into the next stake batch
    /// And the contract is locked
    /// When the account tries to withdraw funds from the batch
    /// Then the funds are transferred back to the account
    #[test]
    fn withdraw_all_funds_from_stake_batch_while_stake_batch_run_locked_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();
        contract.run_stake_batch_locked = true;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_all_funds_from_stake_batch();

        {
            let receipts = deserialize_receipts(&env::created_receipts());
            println!("{:#?}", &receipts);
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, 10 * YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert!(account.next_stake_batch.is_none());
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn withdraw_all_funds_from_stake_batch_while_stake_batch_run_locked_and_stake_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_stake_batch_locked = true;

        testing_env!(context.clone());
        contract.withdraw_all_funds_from_stake_batch();
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn withdraw_all_funds_from_stake_batch_while_unstaking_and_stake_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        testing_env!(context.clone());
        contract.withdraw_all_funds_from_stake_batch();
    }

    #[test]
    #[should_panic(expected = "there are no funds in stake batch")]
    fn withdraw_all_funds_from_stake_batch_no_stake_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        testing_env!(context.clone());
        contract.withdraw_all_funds_from_stake_batch();
    }

    #[test]
    fn cancel_pending_redeem_stake_request_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);

        testing_env!(context.clone());
        contract.redeem((10 * YOCTO).into());

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(account.stake.unwrap().amount, (90 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_some());
        assert!(contract.redeem_stake_batch.is_some());

        testing_env!(context.clone());
        assert!(contract.cancel_uncommitted_redeem_stake_batch());
        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(account.stake.unwrap().amount, (100 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());
        assert!(contract.redeem_stake_batch.is_none());
    }

    #[test]
    fn cancel_pending_redeem_stake_request_success_with_funds_remaining_in_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);

        testing_env!(context.clone());
        contract.redeem((10 * YOCTO).into());
        {
            let mut batch = contract.redeem_stake_batch.unwrap();
            batch.add(YOCTO.into());
            contract.redeem_stake_batch = Some(batch);
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(account.stake.unwrap().amount, (90 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_some());
        assert!(contract.redeem_stake_batch.is_some());

        testing_env!(context.clone());
        assert!(contract.cancel_uncommitted_redeem_stake_batch());
        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(account.stake.unwrap().amount, (100 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());
        assert_eq!(
            contract.redeem_stake_batch.unwrap().balance().amount(),
            YOCTO.into()
        );
    }

    #[test]
    fn cancel_pending_redeem_stake_request_while_locked_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);

        testing_env!(context.clone());
        contract.redeem((10 * YOCTO).into());

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        testing_env!(context.clone());
        contract.redeem((10 * YOCTO).into());

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(account.stake.unwrap().amount, (80 * YOCTO).into());
        assert!(account.next_redeem_stake_batch.is_some());
        assert!(contract.next_redeem_stake_batch.is_some());

        testing_env!(context.clone());
        assert!(contract.cancel_uncommitted_redeem_stake_batch());
        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(account.stake.unwrap().amount, (90 * YOCTO).into());
        assert!(account.next_redeem_stake_batch.is_none());
        assert!(contract.next_redeem_stake_batch.is_none());
    }

    #[test]
    fn cancel_pending_redeem_stake_request_while_locked_success_with_other_funds_in_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);

        testing_env!(context.clone());
        contract.redeem((10 * YOCTO).into());

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        testing_env!(context.clone());
        contract.redeem((10 * YOCTO).into());
        {
            let mut batch = contract.next_redeem_stake_batch.unwrap();
            batch.add(YOCTO.into());
            contract.next_redeem_stake_batch = Some(batch);
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(account.stake.unwrap().amount, (80 * YOCTO).into());
        assert!(account.next_redeem_stake_batch.is_some());
        assert!(contract.next_redeem_stake_batch.is_some());

        testing_env!(context.clone());
        assert!(contract.cancel_uncommitted_redeem_stake_batch());
        let account = contract
            .lookup_account(ValidAccountId::try_from(account_id).unwrap())
            .unwrap();
        assert_eq!(account.stake.unwrap().amount, (90 * YOCTO).into());
        assert!(account.next_redeem_stake_batch.is_none());
        assert_eq!(
            contract.next_redeem_stake_batch.unwrap().balance().amount(),
            YOCTO.into()
        );
    }

    #[test]
    fn cancel_pending_redeem_stake_request_no_batches_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        testing_env!(context.clone());
        assert!(!contract.cancel_uncommitted_redeem_stake_batch());
    }

    #[test]
    fn cancel_pending_redeem_stake_request_while_locked_no_next_batch_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        testing_env!(context.clone());

        assert!(!contract.cancel_uncommitted_redeem_stake_batch());
    }

    #[test]
    fn stake_batch_receipt_lookups() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        assert!(contract
            .stake_batch_receipt(contract.batch_id_sequence.into())
            .is_none());

        *contract.batch_id_sequence += 1;
        contract.stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::StakeBatchReceipt::new(YOCTO.into(), contract.stake_token_value),
        );

        assert_eq!(
            contract
                .stake_batch_receipt(contract.batch_id_sequence.into())
                .unwrap()
                .staked_near,
            YOCTO.into()
        );
    }

    #[test]
    fn redeem_stake_batch_receipt_lookups() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        assert!(contract
            .redeem_stake_batch_receipt(contract.batch_id_sequence.into())
            .is_none());

        *contract.batch_id_sequence += 1;
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new(YOCTO.into(), contract.stake_token_value),
        );

        assert_eq!(
            contract
                .redeem_stake_batch_receipt(contract.batch_id_sequence.into())
                .unwrap()
                .redeemed_stake,
            YOCTO.into()
        );
    }
}
