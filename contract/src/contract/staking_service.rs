//required in order for near_bindgen macro to work outside of lib.rs
use crate::core::U256;
use crate::*;
use crate::{
    domain::{self, Account, RedeemLock, RedeemStakeBatch, RegisteredAccount, StakeBatch},
    errors::{
        illegal_state::{
            REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST, REDEEM_STAKE_BATCH_SHOULD_EXIST,
            STAKE_BATCH_SHOULD_EXIST,
        },
        redeeming_stake_errors::NO_REDEEM_STAKE_BATCH_TO_RUN,
        staking_errors::{BLOCKED_BY_BATCH_RUNNING, NO_FUNDS_IN_STAKE_BATCH_TO_WITHDRAW},
        staking_service::{
            BATCH_BALANCE_INSUFFICIENT, DEPOSIT_REQUIRED_FOR_STAKE,
            INSUFFICIENT_STAKE_FOR_REDEEM_REQUEST, ZERO_REDEEM_AMOUNT,
        },
    },
    interface::{
        staking_service::events, BatchId, RedeemStakeBatchReceipt, StakingService, YoctoNear,
        YoctoStake,
    },
    near::{log, YOCTO},
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
        let mut account = self.predecessor_registered_account();

        let near_amount = env::attached_deposit().into();
        let batch_id = self.deposit_near_for_account_to_stake(&mut account, near_amount);

        self.check_min_required_near_deposit(&account, batch_id);

        self.save_registered_account(&account);
        self.log_stake_batch(batch_id);
        batch_id.into()
    }

    /// runs the stake batch
    ///
    /// logical workflow:
    /// 1. lock the contract
    /// 2. check if liquidity is needed
    /// 3. get account from staking pool
    /// 4. deposit and stake NEAR funds
    ///    - deposit and stake any liquidity that is not needed
    /// 5. create stake batch receipt
    /// 6. update STAKE token supply
    /// 7. unlock contract
    fn stake(&mut self) -> Promise {
        assert!(self.can_run_batch(), BLOCKED_BY_BATCH_RUNNING);
        let batch = self.stake_batch.expect(STAKE_BATCH_SHOULD_EXIST);

        self.run_stake_batch_locked = true;

        self.distribute_earnings();

        if self.is_liquidity_needed() {
            self.staking_pool_promise()
                .get_account()
                .promise()
                .then(self.invoke_on_run_stake_batch())
                .then(self.invoke_release_run_stake_batch_lock())
        } else {
            // if liquidity is not needed, then lets stake it
            // NOTE: liquidity belongs to the stakers - some will leak over when we withdraw all from
            //       the staking pool because of the shares rounding issue on the staking pool side
            let stake_amount = batch.balance().amount() + self.near_liquidity_pool;
            self.near_liquidity_pool = 0.into();
            self.staking_pool_promise()
                .deposit_and_stake(stake_amount)
                .get_account()
                .promise()
                .then(self.invoke_on_deposit_and_stake(None))
                .then(self.invoke_release_run_stake_batch_lock())
        }
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

    fn withdraw_from_stake_batch(&mut self, amount: YoctoNear) {
        let mut account = self.predecessor_registered_account();
        self.claim_receipt_funds(&mut account);

        if let Some(mut batch) = account.next_stake_batch {
            let amount = amount.into();
            let batch_id = batch.id();

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
                self.check_stake_batch_min_required_near_balance(batch);
                account.next_stake_batch = Some(batch);
            }
            self.save_registered_account(&account);
            Promise::new(env::predecessor_account_id()).transfer(amount.value());
            self.log_stake_batch(batch_id);
            return;
        }

        if let Some(mut batch) = account.stake_batch {
            assert!(self.can_run_batch(), BLOCKED_BY_BATCH_RUNNING);

            let amount = amount.into();
            let batch_id = batch.id();

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
            self.save_registered_account(&account);
            Promise::new(env::predecessor_account_id()).transfer(amount.value());
            self.log_stake_batch(batch_id);
            return;
        }

        panic!(NO_FUNDS_IN_STAKE_BATCH_TO_WITHDRAW);
    }

    fn withdraw_all_from_stake_batch(&mut self) -> YoctoNear {
        let mut account = self.predecessor_registered_account();
        self.claim_receipt_funds(&mut account);

        if let Some(batch) = account.next_stake_batch {
            let amount = batch.balance().amount();
            let batch_id = batch.id();

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
            self.save_registered_account(&account);
            Promise::new(env::predecessor_account_id()).transfer(amount.value());
            self.log_stake_batch(batch_id);
            return amount.into();
        }

        if let Some(batch) = account.stake_batch {
            assert!(self.can_run_batch(), BLOCKED_BY_BATCH_RUNNING);

            let amount = batch.balance().amount();
            let batch_id = batch.id();

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
            self.save_registered_account(&account);
            Promise::new(env::predecessor_account_id()).transfer(amount.value());
            self.log_stake_batch(batch_id);
            return amount.into();
        }

        0.into()
    }

    fn redeem(&mut self, amount: YoctoStake) -> BatchId {
        let mut account = self.predecessor_registered_account();
        let batch_id = self.redeem_stake_for_account(&mut account, amount.into());
        self.save_registered_account(&account);
        self.log_redeem_stake_batch(batch_id.clone().into());
        batch_id
    }

    fn redeem_all(&mut self) -> Option<BatchId> {
        let mut account = self.predecessor_registered_account();
        self.claim_receipt_funds(&mut account);
        account.stake.map(|stake| {
            let amount = stake.amount();
            let batch_id = self.redeem_stake_for_account(&mut account, amount);
            self.save_registered_account(&account);
            self.log_redeem_stake_batch(batch_id.clone().into());
            batch_id
        })
    }

    fn remove_all_from_redeem_stake_batch(&mut self) -> YoctoStake {
        let mut account = self.predecessor_registered_account();
        self.claim_receipt_funds(&mut account);

        if self.run_redeem_stake_batch_lock.is_none() {
            if let Some(batch) = account.redeem_stake_batch {
                let amount = batch.balance().amount();
                let batch_id = batch.id();

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
                self.save_registered_account(&account);
                self.log_redeem_stake_batch(batch_id);
                return amount.into();
            }
        } else if let Some(batch) = account.next_redeem_stake_batch {
            let amount = batch.balance().amount();
            let batch_id = batch.id();

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
            self.save_registered_account(&account);
            self.log_redeem_stake_batch(batch_id);
            return amount.into();
        }

        0.into()
    }

    fn remove_from_redeem_stake_batch(&mut self, amount: YoctoStake) {
        let mut account = self.predecessor_registered_account();
        self.claim_receipt_funds(&mut account);

        if self.run_redeem_stake_batch_lock.is_none() {
            if let Some(mut batch) = account.redeem_stake_batch {
                let amount: domain::YoctoStake = amount.into();
                assert!(
                    amount <= batch.balance().amount(),
                    BATCH_BALANCE_INSUFFICIENT
                );

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
                if batch.remove(amount).value() == 0 {
                    account.redeem_stake_batch = None;
                } else {
                    account.redeem_stake_batch = Some(batch);
                }
                self.save_registered_account(&account);
                self.log_redeem_stake_batch(batch.id());
            }
        } else if let Some(mut batch) = account.next_redeem_stake_batch {
            let amount: domain::YoctoStake = amount.into();
            assert!(
                amount <= batch.balance().amount(),
                BATCH_BALANCE_INSUFFICIENT
            );

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
            if batch.remove(amount).value() == 0 {
                account.next_redeem_stake_batch = None;
            } else {
                account.next_redeem_stake_batch = Some(batch);
            }
            self.save_registered_account(&account);
            self.log_redeem_stake_batch(batch.id());
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

                self.staking_pool_promise()
                    .get_account()
                    .promise()
                    .then(self.invoke_on_run_redeem_stake_batch())
                    .then(self.invoke_release_run_redeem_stake_batch_unstaking_lock())
            }
            Some(RedeemLock::PendingWithdrawal) => self
                .staking_pool_promise()
                .get_account()
                .promise()
                .then(self.invoke_on_redeeming_stake_pending_withdrawal()),
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

    fn redeem_all_and_unstake(&mut self) -> PromiseOrValue<Option<BatchId>> {
        match self.redeem_all() {
            None => PromiseOrValue::Value(None),
            Some(batch_id) => {
                if self.can_unstake() {
                    PromiseOrValue::Promise(self.unstake())
                } else {
                    PromiseOrValue::Value(Some(batch_id))
                }
            }
        }
    }

    fn pending_withdrawal(&self) -> Option<RedeemStakeBatchReceipt> {
        self.get_pending_withdrawal()
            .map(RedeemStakeBatchReceipt::from)
    }

    fn claim_receipts(&mut self) {
        let mut account = self.predecessor_registered_account();
        self.claim_receipt_funds(&mut account);
    }

    fn withdraw(&mut self, amount: interface::YoctoNear) {
        let mut account = self.predecessor_registered_account();
        self.withdraw_near_funds(&mut account, amount.into());
    }

    fn withdraw_all(&mut self) -> interface::YoctoNear {
        let mut account = self.predecessor_registered_account();
        self.claim_receipt_funds(&mut account);
        match account.near {
            None => 0.into(),
            Some(balance) => {
                self.withdraw_near_funds(&mut account, balance.amount());
                balance.amount().into()
            }
        }
    }

    fn transfer_near(&mut self, recipient: ValidAccountId, amount: interface::YoctoNear) {
        let mut account = self.predecessor_registered_account();
        self.transfer_near_funds(&mut account, amount.into(), recipient);
    }

    fn transfer_all_near(&mut self, recipient: ValidAccountId) -> interface::YoctoNear {
        let mut account = self.predecessor_registered_account();
        self.claim_receipt_funds(&mut account);
        match account.near {
            None => 0.into(),
            Some(balance) => {
                self.transfer_near_funds(&mut account, balance.amount(), recipient);
                balance.amount().into()
            }
        }
    }

    fn min_required_deposit_to_stake(&self) -> YoctoNear {
        self.min_required_near_deposit().into()
    }
}

// staking pool func call invocations
impl StakeTokenContract {
    fn log_stake_batch(&self, batch_id: domain::BatchId) {
        if let Some(batch) = self.stake_batch {
            if batch_id == batch.id() {
                log(events::StakeBatch::from(batch));
            }
        } else if let Some(batch) = self.next_stake_batch {
            if batch_id == batch.id() {
                log(events::StakeBatch::from(batch));
            }
        } else {
            log(events::StakeBatchCancelled {
                batch_id: batch_id.value(),
            });
        }
    }

    fn log_redeem_stake_batch(&self, batch_id: domain::BatchId) {
        if let Some(batch) = self.redeem_stake_batch {
            if batch_id == batch.id() {
                log(events::RedeemStakeBatch::from(batch));
            }
        } else if let Some(batch) = self.next_redeem_stake_batch {
            if batch_id == batch.id() {
                log(events::RedeemStakeBatch::from(batch));
            }
        } else {
            log(events::RedeemStakeBatchCancelled {
                batch_id: batch_id.value(),
            });
        }
    }
}

/// NEAR transfers
impl StakeTokenContract {
    fn withdraw_near_funds(&mut self, account: &mut RegisteredAccount, amount: domain::YoctoNear) {
        self.claim_receipt_funds(account);
        account.apply_near_debit(amount);
        self.save_registered_account(&account);
        // check if there are enough funds to fulfill the request - if not then draw from liquidity
        if self.total_near.amount() < amount {
            // access liquidity
            // NOTE: will panic if there are not enough funds in liquidity pool
            //       - should never panic unless there is a bug
            let difference = amount - self.total_near.amount();
            self.near_liquidity_pool -= difference;
            self.total_near.credit(difference);
        }
        self.total_near.debit(amount);
        Promise::new(env::predecessor_account_id()).transfer(amount.value());
    }

    fn transfer_near_funds(
        &mut self,
        account: &mut RegisteredAccount,
        amount: domain::YoctoNear,
        recipient: ValidAccountId,
    ) {
        self.claim_receipt_funds(account);
        account.apply_near_debit(amount);
        self.save_registered_account(&account);
        // check if there are enough funds to fulfill the request - if not then draw from liquidity
        if self.total_near.amount() < amount {
            // access liquidity
            // NOTE: will panic if there are not enough funds in liquidity pool
            //       - should never panic unless there is a bug
            let difference = amount - self.total_near.amount();
            self.near_liquidity_pool -= difference;
            self.total_near.credit(difference);
        }
        self.total_near.debit(amount);
        Promise::new(recipient.as_ref().to_string()).transfer(amount.value());
    }
}

impl StakeTokenContract {
    /// check that batch NEAR amount will issue at least 1 yoctoSTAKE
    /// we never want to issue 0 yoctoSTAKE tokens if NEAR is deposited and staked
    ///
    /// the min required NEAR deposit is calculated using the cached STAKE token value
    /// thus, to be on the safe side, we will require that minimum amount of NEAR deposit should be
    /// enough for 1000 yoctoSTAKE
    fn check_min_required_near_deposit(&self, account: &Account, batch_id: domain::BatchId) {
        if let Some(batch) = account.stake_batch(batch_id) {
            self.check_stake_batch_min_required_near_balance(batch)
        }
    }

    fn check_stake_batch_min_required_near_balance(&self, batch: StakeBatch) {
        let min_required_near_deposit = self.min_required_near_deposit();
        assert!(
            batch.balance().amount() >= min_required_near_deposit,
            "minimum required NEAR deposit is: {}",
            min_required_near_deposit
        );
    }

    fn min_required_near_deposit(&self) -> domain::YoctoNear {
        self.stake_token_value.stake_to_near(1000.into())
    }

    pub(crate) fn get_pending_withdrawal(&self) -> Option<domain::RedeemStakeBatchReceipt> {
        self.redeem_stake_batch
            .map(|batch| self.redeem_stake_batch_receipts.get(&batch.id()))
            .flatten()
    }

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
        account: &mut RegisteredAccount,
        amount: domain::YoctoNear,
    ) -> domain::BatchId {
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

            account_batch.id()
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

            account_batch.id()
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
        account: &mut RegisteredAccount,
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

        match self.run_redeem_stake_batch_lock {
            // use current batch
            None => {
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
            }
            // use next batch
            _ => {
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
    }

    fn new_redeem_stake_batch(&mut self) -> RedeemStakeBatch {
        *self.batch_id_sequence += 1;
        self.batch_id_sequence.new_redeem_stake_batch()
    }

    /// NOTE: the account is saved to storage if funds were claimed
    pub(crate) fn claim_receipt_funds(&mut self, account: &mut RegisteredAccount) {
        let claimed_stake_tokens = self.claim_stake_batch_receipts(&mut account.account);
        let claimed_near_tokens = self.claim_redeem_stake_batch_receipts(&mut account.account);
        let funds_were_claimed = claimed_stake_tokens || claimed_near_tokens;
        if funds_were_claimed {
            self.save_registered_account(&account);
        }
    }

    /// the purpose of this method is to to compute the account's STAKE balance taking into consideration
    /// that there may be unclaimed receipts on the account
    /// - this enables the latest account info to be returned within the context of a contract 'view'
    ///   call - no receipts are physically claimed, i.e., contract state does not change
    pub(crate) fn apply_receipt_funds_for_view(&self, account: &Account) -> Account {
        let mut account = account.clone();

        {
            fn apply_stake_credit(
                account: &mut Account,
                batch: StakeBatch,
                receipt: StakeBatchReceipt,
            ) {
                let staked_near = batch.balance().amount();
                let stake = receipt.stake_token_value().near_to_stake(staked_near);
                account.apply_stake_credit(stake);
            }

            if let Some(batch) = account.stake_batch {
                if let Some(receipt) = self.stake_batch_receipts.get(&batch.id()) {
                    apply_stake_credit(&mut account, batch, receipt);
                    account.stake_batch = None;
                }
            }

            if let Some(batch) = account.next_stake_batch {
                if let Some(receipt) = self.stake_batch_receipts.get(&batch.id()) {
                    apply_stake_credit(&mut account, batch, receipt);
                    account.next_stake_batch = None;
                }
            }
        }

        {
            fn apply_near_credit(
                account: &mut Account,
                batch: RedeemStakeBatch,
                receipt: domain::RedeemStakeBatchReceipt,
            ) {
                let redeemed_stake = batch.balance().amount();
                let near = receipt.stake_token_value().stake_to_near(redeemed_stake);
                account.apply_near_credit(near);
            }

            if let Some(RedeemLock::PendingWithdrawal) = self.run_redeem_stake_batch_lock {
                // NEAR funds cannot be claimed from a receipt that is pending withdrawal from the staking pool
                let batch_pending_withdrawal_id = self.redeem_stake_batch.as_ref().unwrap().id();

                if let Some(batch) = account.redeem_stake_batch {
                    if batch_pending_withdrawal_id != batch.id() {
                        if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                            apply_near_credit(&mut account, batch, receipt);
                            account.redeem_stake_batch = None
                        }
                    }
                }

                if let Some(batch) = account.next_redeem_stake_batch {
                    if batch_pending_withdrawal_id != batch.id() {
                        if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                            apply_near_credit(&mut account, batch, receipt);
                            account.next_redeem_stake_batch = None
                        }
                    }
                }
            } else {
                if let Some(batch) = account.redeem_stake_batch {
                    if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                        apply_near_credit(&mut account, batch, receipt);
                        account.redeem_stake_batch = None
                    }
                }

                if let Some(batch) = account.next_redeem_stake_batch {
                    if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                        apply_near_credit(&mut account, batch, receipt);
                        account.next_redeem_stake_batch = None
                    }
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
        // NOTE: while the contract is locked for running a stake batch, all deposits must go into
        //       the next batch
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
            account_batch: domain::RedeemStakeBatch,
            mut receipt: domain::RedeemStakeBatchReceipt,
        ) {
            // how much STAKE did the account redeem in the batch
            let redeemed_stake = account_batch.balance().amount();

            // claim the NEAR tokens for the account
            let near = receipt.stake_token_value().stake_to_near(redeemed_stake);
            account.apply_near_credit(near);

            // track that the NEAR tokens were claimed
            receipt.stake_tokens_redeemed(redeemed_stake);
            if receipt.all_claimed() {
                // then delete the receipt and free the storage
                contract
                    .redeem_stake_batch_receipts
                    .remove(&account_batch.id());
            } else {
                contract
                    .redeem_stake_batch_receipts
                    .insert(&account_batch.id(), &receipt);
            }
        }

        /// for a pending withdrawal, funds can also be claimed against the liquidity pool
        fn claim_redeemed_stake_for_batch_pending_withdrawal(
            contract: &mut StakeTokenContract,
            account: &mut Account,
            account_batch: &mut domain::RedeemStakeBatch,
            mut receipt: domain::RedeemStakeBatchReceipt,
        ) {
            // how much STAKE did the account redeem in the batch
            let redeemed_stake = account_batch.balance().amount();

            let redeemed_stake_near_value =
                receipt.stake_token_value().stake_to_near(redeemed_stake);
            let claimed_near = if contract.near_liquidity_pool >= redeemed_stake_near_value {
                redeemed_stake_near_value
            } else {
                contract.near_liquidity_pool
            };
            let redeemable_stake = receipt.stake_token_value().near_to_stake(claimed_near);
            account_batch.remove(redeemable_stake);

            // claim the STAKE tokens for the account
            // let near = receipt.stake_token_value().stake_to_near(redeemable_stake);
            account.apply_near_credit(claimed_near);
            contract.near_liquidity_pool -= claimed_near;
            contract.total_near.credit(claimed_near);

            // track that the STAKE tokens were claimed
            receipt.stake_tokens_redeemed(redeemable_stake);
            if receipt.all_claimed() {
                // this means that effectively all funds have been withdrawn
                // which means we need to finalize the redeem workflow
                contract
                    .redeem_stake_batch_receipts
                    .remove(&account_batch.id());
                contract.run_redeem_stake_batch_lock = None;
                contract.pop_redeem_stake_batch();
            } else {
                contract
                    .redeem_stake_batch_receipts
                    .insert(&account_batch.id(), &receipt);
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
        // NOTE: while a contract is locked, all redeem requests must be collected in the next batch
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

    /// returns a new [StakeTokenValue](crate::domain::StakeTokenValue) updated with the new staked
    /// NEAR balance.
    pub(crate) fn update_stake_token_value(
        &mut self,
        total_staked_near_balance: domain::YoctoNear,
    ) {
        let new_stake_token_value = domain::StakeTokenValue::new(
            domain::BlockTimeHeight::from_env(),
            total_staked_near_balance,
            self.total_stake.amount(),
        );

        // the new STAKE token value should never be less than the current STAKE token value, unless
        // the total staked NEAR balance is zero
        // - when NEAR is staked, the staking pool converts the NEAR into shares. Because of rounding,
        //   not all staked NEAR gets converted into shares, and some is left behind as unstaked in
        //   the staking pool. In the example below 0.25 NEAR was deposited to be staked, however
        //   after converting the NEAR to shares, there were 5 yoctoNEAR left over that remained
        //   as unstaked:
        //
        // Log [stake.oysterpack.testnet]: @stake.oysterpack.testnet deposited 250000000000000000000000. New unstaked balance is 654566211093653841620326
        // Log [stake.oysterpack.testnet]: @stake.oysterpack.testnet staking 249999999999999999999995. Received 13510178747482595266283 new staking shares. Total 404566211093653841620331 unstaked balance and 1146041341904922841152939 staking shares
        //
        // Thus, if we see that the STAKE value ticks down, we need to compensate the [total_staked_near_balance]
        // because the STAKE value should never decrease.
        //
        // How can this happen? When we withdraw unstaked funds, we do a withdraw all, which will
        // withdraw unstaked NEAR that should have been staked but couldn't because of the share conversion
        // rounding. When we need to compensate, then we need to add the compensation to the liquidity
        // to balance everything out.
        let new_stake_near_value = new_stake_token_value.stake_to_near(YOCTO.into());
        let current_stake_near_value = self.stake_token_value.stake_to_near(YOCTO.into());
        self.stake_token_value = if new_stake_near_value >= current_stake_near_value
            || total_staked_near_balance.value() == 0
        {
            new_stake_token_value
        } else {
            let current_stake_near_value: U256 = U256::from(current_stake_near_value);
            let total_stake_supply: U256 = U256::from(self.total_stake.amount());
            let total_staked_near_balance: U256 = U256::from(total_staked_near_balance.value());
            // (staked_near_compensation + total_staked_near_balance)    current_stake_near_value
            // ------------------------------------------------------ =  ------------------------
            //           total_staked_near_balance                               YOCTO
            let staked_near_compensation = (current_stake_near_value * total_stake_supply
                / U256::from(YOCTO))
                - total_staked_near_balance;
            // compensation needs to be added back to NEAR liquidity to rebalance the amounts
            *self.near_liquidity_pool += staked_near_compensation.as_u128();
            log(events::NearLiquidityAdded {
                amount: staked_near_compensation.as_u128(),
                balance: self.near_liquidity_pool.value(),
            });
            domain::StakeTokenValue::new(
                new_stake_token_value.block_time_height(),
                (total_staked_near_balance + staked_near_compensation)
                    .as_u128()
                    .into(),
                self.total_stake.amount(),
            )
        }
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
pub trait ExtStakingWorkflowCallbacks {
    /// callback for getting staked balance from staking pool as part of stake batch processing workflow
    ///
    /// ## Success Workflow
    /// 1. Check if liquidity is needed
    /// 2. deposit and stake funds with staking pool
    /// 3. then get account from staking pool
    /// 4. then invoke [on_deposit_and_stake] callback
    fn on_run_stake_batch(
        &mut self,
        #[callback] staking_pool_account: StakingPoolAccount,
    ) -> Promise;

    /// ## Success Workflow
    /// 1. update the stake token value
    /// 2. store the stake batch receipt
    /// 3. update the STAKE token supply with the new STAKE tokens that were issued
    fn on_deposit_and_stake(
        &mut self,
        near_liquidity: Option<interface::YoctoNear>,
        #[callback] staking_pool_account: StakingPoolAccount,
    );

    /// defined on [Operator] interface
    fn release_run_stake_batch_lock(&mut self);
}

#[cfg(test)]
mod test_stake {
    use super::*;

    use crate::{near::YOCTO, test_utils::*};
    use near_sdk::{testing_env, MockedBlockchain};

    /// Given the contract has a stake batch
    /// And the contract is not locked
    /// When the stake batch is run
    /// Then it generates to FunctionCall receipts:
    ///   1. deposit and stake into the staking pool
    ///   1.1. get account from staking pool
    ///   2. invoke `on_deposit_and_stake` callback
    ///   3. and finally a callback into this contract to unlock the contract
    #[test]
    fn with_no_locks() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
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

        let receipts: Vec<Receipt> = deserialize_receipts();
        assert_eq!(receipts.len(), 3);

        {
            let receipt = &receipts[0];
            assert_eq!(receipt.actions.len(), 2);
            {
                let action = &receipt.actions[0];
                match action {
                    Action::FunctionCall { method_name, .. } => {
                        assert_eq!(method_name, "deposit_and_stake")
                    }
                    _ => panic!("expected `deposit_and_stake` func call on staking pool"),
                }
            }
            {
                let action = &receipt.actions[1];
                match action {
                    Action::FunctionCall { method_name, .. } => {
                        assert_eq!(method_name, "get_account")
                    }
                    _ => panic!("expected `get_account` func call on staking pool"),
                }
            }
        }

        {
            let receipt = &receipts[1];
            let action = &receipt.actions[0];
            match action {
                Action::FunctionCall { method_name, .. } => {
                    assert_eq!(method_name, "on_deposit_and_stake")
                }
                _ => panic!("expected `get_account` func call on staking pool"),
            }
        }

        {
            let receipt = &receipts[2];
            let action = &receipt.actions[0];
            match action {
                Action::FunctionCall { method_name, .. } => {
                    assert_eq!(method_name, "release_run_stake_batch_lock")
                }
                _ => panic!("expected `release_run_stake_batch_lock` callback"),
            }
        }
    }

    /// Given there is a pending withdrawal
    /// And the contract is not locked
    /// When the stake batch is run
    /// Then it generates to FunctionCall receipts:
    ///   1. get account from staking pool
    ///   2. and then to callback into this contract - on_run_stake_batch
    ///   3. and finally a callback into this contract to unlock the contract
    #[test]
    fn with_pending_withdrawal() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        *contract.batch_id_sequence += 1;
        let redeem_stake_batch =
            domain::RedeemStakeBatch::new(contract.batch_id_sequence, YOCTO.into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        let receipt = redeem_stake_batch.create_receipt(contract.stake_token_value);
        contract
            .redeem_stake_batch_receipts
            .insert(&contract.batch_id_sequence, &receipt);

        context.prepaid_gas = 10u64.pow(18);
        testing_env!(context.clone());
        contract.stake();

        let receipts = deserialize_receipts();
        assert_eq!(receipts.len(), 3);

        {
            let receipt = &receipts[0];
            let action = &receipt.actions[0];
            match action {
                Action::FunctionCall { method_name, .. } => {
                    assert_eq!(method_name, "get_account")
                }
                _ => panic!("expected `deposit_and_stake` func call on staking pool"),
            }
        }

        {
            let receipt = &receipts[1];
            let action = &receipt.actions[0];
            match action {
                Action::FunctionCall { method_name, .. } => {
                    assert_eq!(method_name, "on_run_stake_batch")
                }
                _ => panic!("expected `get_account` func call on staking pool"),
            }
        }

        {
            let receipt = &receipts[2];
            let action = &receipt.actions[0];
            match action {
                Action::FunctionCall { method_name, .. } => {
                    assert_eq!(method_name, "release_run_stake_batch_lock")
                }
                _ => panic!("expected `release_run_stake_batch_lock` callback"),
            }
        }
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn withdraw_funds_from_stake_batch_while_stake_batch_run_locked_and_stake_batch() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_stake_batch_locked = true;

        testing_env!(context.clone());
        contract.withdraw_from_stake_batch(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn withdraw_funds_from_stake_batch_while_unstaking_and_stake_batch() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        testing_env!(context.clone());
        contract.withdraw_from_stake_batch(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "there are no funds in stake batch")]
    fn withdraw_funds_from_stake_batch_no_stake_batch() {
        let mut test_context = TestContext::with_registered_account();
        let context = test_context.context.clone();
        let contract = &mut test_context.contract;

        testing_env!(context.clone());
        contract.withdraw_from_stake_batch(YOCTO.into());
    }
}

#[cfg(test)]
mod test_withdraw_from_stake_batch {
    use super::*;

    use crate::{interface::AccountManagement, near::YOCTO, test_utils::*};
    use near_sdk::{json_types::ValidAccountId, testing_env, MockedBlockchain};
    use std::convert::TryFrom;

    /// Given an account has deposited funds into a stake batch
    /// And the contract is not locked
    /// When the account tries to withdraw funds from the batch
    /// Then the funds are transferred back to the account
    #[test]
    fn account_has_uncommitted_stake_batch() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_from_stake_batch(YOCTO.into());

        {
            let receipts = deserialize_receipts();
            println!("{:#?}", &receipts);
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, test_context.account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
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
    fn withdraw_all_funds_from_batch_specifying_exact_amount() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_from_stake_batch(context.attached_deposit.into());

        {
            let receipts = deserialize_receipts();
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, test_context.account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, context.attached_deposit),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
    }

    /// Given an account has deposited funds into the next stake batch
    /// And the contract is locked
    /// When the account tries to withdraw funds from the batch
    /// Then the funds are transferred back to the account
    #[test]
    fn while_stake_batch_is_locked_withdraw_partial() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;
        contract.run_stake_batch_locked = true;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_from_stake_batch(YOCTO.into());

        {
            let receipts = deserialize_receipts();
            println!("{:#?}", &receipts);
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, test_context.account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
            .unwrap();
        assert_eq!(
            account.next_stake_batch.unwrap().balance.amount.value(),
            (9 * YOCTO)
        );
    }

    /// Given an account has deposited funds into the next stake batch
    /// And the contract is locked
    /// When the account tries to withdraw all funds from the batch
    /// Then the funds are transferred back to the account
    /// And the batch is deleted on the account
    #[test]
    fn while_stake_batch_is_locked_withdraw_all() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;
        contract.run_stake_batch_locked = true;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_from_stake_batch(context.attached_deposit.into());

        {
            let receipts = deserialize_receipts();
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, test_context.account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, context.attached_deposit),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
            .unwrap();
        assert!(account.next_stake_batch.is_none());
    }
}

#[cfg(test)]
mod test_withdraw_all_from_stake_batch {
    use super::*;

    use crate::{interface::AccountManagement, near::YOCTO, test_utils::*};
    use near_sdk::{json_types::ValidAccountId, testing_env, MockedBlockchain};
    use std::convert::TryFrom;

    /// Given an account has deposited funds into the next stake batch
    /// And the contract is locked
    /// When the account tries to withdraw funds from the batch
    /// Then the funds are transferred back to the account
    #[test]
    fn while_stake_batch_is_locked() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;
        contract.run_stake_batch_locked = true;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        testing_env!(context.clone());
        contract.withdraw_all_from_stake_batch();

        {
            let receipts = deserialize_receipts();
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, test_context.account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, 10 * YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
            .unwrap();
        assert!(account.next_stake_batch.is_none());
    }

    #[test]
    fn while_stake_batch_is_locked_with_other_funds_batch() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;
        contract.run_stake_batch_locked = true;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();
        assert!(contract.next_stake_batch.is_some());
        if let Some(batch) = contract.next_stake_batch.as_mut() {
            batch.add(YOCTO.into());
        }

        testing_env!(context.clone());
        contract.withdraw_all_from_stake_batch();

        {
            let receipts = deserialize_receipts();
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, test_context.account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, 10 * YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
            .unwrap();
        assert!(account.next_stake_batch.is_none());
        assert_eq!(
            contract.next_stake_batch.unwrap().balance().amount(),
            YOCTO.into()
        );
    }

    #[test]
    fn from_uncommitted_stake_batch() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();
        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_some());
        assert!(contract.stake_batch.is_some());

        testing_env!(context.clone());
        contract.withdraw_all_from_stake_batch();

        {
            let receipts = deserialize_receipts();
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, test_context.account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, 10 * YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        assert!(contract.stake_batch.is_none());
    }

    #[test]
    fn from_uncommitted_stake_batch_with_other_funds_batched() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();
        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_some());
        assert!(contract.stake_batch.is_some());
        if let Some(batch) = contract.stake_batch.as_mut() {
            batch.add(YOCTO.into());
        }

        testing_env!(context.clone());
        contract.withdraw_all_from_stake_batch();

        {
            let receipts = deserialize_receipts();
            assert_eq!(receipts.len(), 1);
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, test_context.account_id);
            match receipt.actions.first().unwrap() {
                Action::Transfer { deposit } => assert_eq!(*deposit, 10 * YOCTO),
                _ => panic!("unexpected action type"),
            }
        }

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        assert_eq!(
            contract.stake_batch.unwrap().balance().amount(),
            YOCTO.into()
        );
    }

    #[test]
    fn with_no_stake_batch() {
        let mut test_context = TestContext::with_registered_account();
        let context = test_context.context.clone();
        let contract = &mut test_context.contract;

        testing_env!(context.clone());
        assert_eq!(contract.withdraw_all_from_stake_batch().value(), 0);
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn withdraw_all_funds_from_stake_batch_while_unstaking() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        testing_env!(context.clone());
        contract.withdraw_all_from_stake_batch();
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn withdraw_all_funds_from_stake_batch_while_stake_batch_is_locked() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_stake_batch_locked = true;

        testing_env!(context.clone());
        contract.withdraw_all_from_stake_batch();
    }
}

#[cfg(test)]
mod test_withdraw {
    use super::*;

    use crate::{near::YOCTO, test_utils::*};
    use std::ops::DerefMut;

    #[test]
    fn partial_funds() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // Given the account has some NEAR balance
        let mut account = contract.registered_account(test_context.account_id);
        account.deref_mut().apply_near_credit((10 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_near.credit(account.near.unwrap().amount());

        // When partial funds are withdrawn
        contract.withdraw((5 * YOCTO).into());
        // Assert that the account NEAR balance was debited
        let account = contract.registered_account(test_context.account_id);
        assert_eq!(*account.near.unwrap().amount(), (5 * YOCTO).into());
    }

    #[test]
    #[should_panic(expected = "account has zero NEAR balance")]
    fn with_no_near_funds() {
        let mut test_context = TestContext::with_registered_account();
        test_context.contract.withdraw((50 * YOCTO).into());
    }

    #[test]
    #[should_panic(expected = "account NEAR balance is too low to fulfill request")]
    fn with_insufficient_funds() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // Given the account has some NEAR balance
        let mut account = contract.registered_account(test_context.account_id);
        account.deref_mut().apply_near_credit((10 * YOCTO).into());
        contract.save_registered_account(&account);

        contract.withdraw((50 * YOCTO).into());
    }
}

#[cfg(test)]
mod test_withdraw_all {
    use super::*;

    use crate::{near::YOCTO, test_utils::*};
    use std::ops::Deref;

    #[test]
    fn has_near_funds() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // Given the account has some NEAR balance
        let mut account = contract.registered_account(test_context.account_id);
        account.apply_near_credit((10 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_near.credit(account.near.unwrap().amount());

        contract.withdraw_all();
        // Assert that the account NEAR balance was debited
        let account = contract.registered_account(test_context.account_id);
        assert!(account.deref().near.is_none());
    }

    #[test]
    fn has_near_funds_in_unclaimed_receipts() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // Given the account has some NEAR balance
        let mut account = contract.registered_account(test_context.account_id);
        *contract.batch_id_sequence += 1;
        account.account.redeem_stake_batch = Some(RedeemStakeBatch::new(
            contract.batch_id_sequence,
            YOCTO.into(),
        ));
        contract.save_registered_account(&account);
        contract.total_near.credit(YOCTO.into());
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new(YOCTO.into(), contract.stake_token_value),
        );

        contract.withdraw_all();
        // Assert that the account NEAR balance was debited
        let account = contract.registered_account(test_context.account_id);
        assert!(account.account.near.is_none());
    }

    #[test]
    fn with_no_near_funds() {
        // Arrange
        let mut context = TestContext::with_registered_account();
        let contract = &mut context.contract;

        // Act
        let amount = contract.withdraw_all();

        // Assert
        assert_eq!(amount.value(), 0);
    }
}

#[cfg(test)]
mod test_claim_receipts {
    use super::*;

    use crate::domain::BlockTimeHeight;
    use crate::test_utils::*;
    use crate::{interface::AccountManagement, near::YOCTO};
    use near_sdk::{testing_env, MockedBlockchain};
    use std::convert::TryInto;

    #[test]
    #[should_panic(expected = "account is not registered")]
    fn when_account_is_not_registered() {
        // Arrange
        let mut test_context = TestContext::new();
        let contract = &mut test_context.contract;

        // Act
        contract.claim_receipts();
    }

    /// Given the account has no funds in stake batches
    /// When funds are claimed
    /// Then there should be no effect
    #[test]
    fn when_account_has_no_batches() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // Act
        contract.claim_receipts();
    }

    /// Given the account has funds in the stake batch
    /// And there is no receipt for the batch
    /// When funds are claimed
    /// Then there should be no effect on the account
    #[test]
    fn when_account_has_funds_in_unprocessed_stake_batch() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // deposit NEAR into StakeBatch
        test_context.context.attached_deposit = YOCTO;
        testing_env!(test_context.context.clone());
        let batch_id = contract.deposit();

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract
            .lookup_account(test_context.account_id.try_into().unwrap())
            .unwrap();
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(stake_batch.id, batch_id.into());
        assert_eq!(stake_batch.balance.amount, YOCTO.into());
        assert!(account.stake.is_none());
    }

    /// Given the account has funds in the stake batch
    /// And there is a receipt for the batch with additional funds batched into it
    /// When funds are claimed
    /// Then the STAKE tokens should be credited to the account
    /// And the receipt NEAR balance should have been debited
    #[test]
    fn when_account_has_batch_with_receipt() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;
        let mut context = test_context.context.clone();

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        let batch_id = contract.deposit();
        let batch_id: domain::BatchId = domain::BatchId(batch_id.into());

        // create a receipt for the batch to simulate that the batch has been staked
        {
            let stake_token_value =
                domain::StakeTokenValue::new(Default::default(), YOCTO.into(), YOCTO.into());
            let receipt = domain::StakeBatchReceipt::new(
                (context.attached_deposit * 2).into(), // simulate that other accounts have deposited into the same batch
                stake_token_value,
            );
            contract.stake_batch_receipts.insert(&batch_id, &receipt);
        }

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;
        assert_eq!(
            account.stake.unwrap().amount().value(),
            YOCTO,
            "the funds should have been claimed by the account"
        );
        assert!(
            account.stake_batch.is_none(),
            "stake batch should be set to None"
        );
        let receipt = contract.stake_batch_receipts.get(&batch_id.into()).unwrap();
        assert_eq!(
            receipt.staked_near().value(),
            YOCTO,
            "claiming STAKE tokens should have reduced the near balance on the receipt"
        );
    }

    /// Given the account has funds in the stake batch
    /// And there is a receipt for the batch with exact matching funds
    /// When funds are claimed
    /// Then the STAKE tokens should be credited to the account
    /// And the receipt is deleted
    #[test]
    fn when_all_funds_on_stake_batch_receipt_are_claimed() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;
        let mut context = test_context.context.clone();

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        let batch_id = contract.deposit();
        let batch_id: domain::BatchId = domain::BatchId(batch_id.into());

        let stake_token_value =
            domain::StakeTokenValue::new(Default::default(), YOCTO.into(), YOCTO.into());
        let receipt =
            domain::StakeBatchReceipt::new(context.attached_deposit.into(), stake_token_value);
        contract.stake_batch_receipts.insert(&batch_id, &receipt);

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;

        assert_eq!(
            account.stake.unwrap().amount().value(),
            context.attached_deposit,
            "the funds should have been claimed by the account"
        );
        assert!(
            account.stake_batch.is_none(),
            "stake batch should be set to None"
        );
        assert!(
            contract.stake_batch_receipts.get(&batch_id).is_none(),
            "when all STAKE tokens are claimed, then the receipt should have been deleted"
        );
    }

    /// Given Account::stake_batch and Account::next_stake_batch both have funds
    /// And there are exact receipts for both batches
    /// Then STAKE tokens should be claimed for both
    /// And the receipts should be deleted
    #[test]
    fn when_account_has_stake_batch_and_next_stake_batch_funds_with_receipts() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;
        let mut context = test_context.context.clone();

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        let batch_id = contract.deposit();
        let batch_id_1: domain::BatchId = domain::BatchId(batch_id.into());

        contract.run_stake_batch_locked = true;
        context.attached_deposit = YOCTO * 2;
        testing_env!(context.clone());
        let batch_id = contract.deposit();
        let batch_id_2: domain::BatchId = domain::BatchId(batch_id.into());
        assert_ne!(batch_id_1, batch_id_2);

        {
            let stake_token_value =
                domain::StakeTokenValue::new(Default::default(), YOCTO.into(), YOCTO.into());
            contract.stake_batch_receipts.insert(
                &batch_id_1,
                &domain::StakeBatchReceipt::new(YOCTO.into(), stake_token_value),
            );
            contract.stake_batch_receipts.insert(
                &batch_id_2,
                &domain::StakeBatchReceipt::new((YOCTO * 2).into(), stake_token_value),
            );
        }

        contract.run_stake_batch_locked = false;

        // Act
        contract.claim_receipts();

        // Assert
        assert!(contract.stake_batch_receipts.get(&batch_id_1).is_none());
        assert!(contract.stake_batch_receipts.get(&batch_id_2).is_none());

        let account = contract.predecessor_registered_account().account;
        // and the account batches have been cleared
        assert!(account.stake_batch.is_none());
        assert!(account.next_stake_batch.is_none());
        // and the STAKE tokens were claimed and credited to the account
        assert_eq!(account.stake.unwrap().amount().value(), 3 * YOCTO);
    }

    #[test]
    fn when_account_has_stake_batch_and_next_stake_batch_funds_with_receipt_for_stake_batch() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;
        let mut context = test_context.context.clone();

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        let batch_id = contract.deposit();
        let batch_id_1: domain::BatchId = domain::BatchId(batch_id.into());

        contract.run_stake_batch_locked = true;
        context.attached_deposit = YOCTO * 2;
        testing_env!(context.clone());
        let batch_id = contract.deposit();
        let batch_id_2: domain::BatchId = domain::BatchId(batch_id.into());
        assert_ne!(batch_id_1, batch_id_2);

        {
            let stake_token_value =
                domain::StakeTokenValue::new(Default::default(), YOCTO.into(), YOCTO.into());
            contract.stake_batch_receipts.insert(
                &batch_id_1,
                &domain::StakeBatchReceipt::new(YOCTO.into(), stake_token_value),
            );
        }

        contract.run_stake_batch_locked = false;

        // Act
        contract.claim_receipts();

        // Assert
        assert!(contract.stake_batch_receipts.get(&batch_id_1).is_none());

        let account = contract.predecessor_registered_account().account;
        // and the account batches have been cleared
        assert_eq!(account.stake_batch.unwrap().id(), batch_id_2);
        assert!(account.next_stake_batch.is_none());
        // and the STAKE tokens were claimed and credited to the account
        assert_eq!(account.stake.unwrap().amount().value(), YOCTO);
    }

    /// Given an account has redeemed STAKE
    /// And the batch has completed
    /// Then the account can claim the NEAR funds
    #[test]
    fn when_account_has_redeem_stake_batch_with_receipt() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        let mut account = contract.predecessor_registered_account();
        account.apply_stake_credit(YOCTO.into());
        contract.save_registered_account(&account);
        let batch_id = contract
            .redeem_all()
            .map(|batch_id| domain::BatchId(batch_id.into()))
            .unwrap();

        contract.redeem_stake_batch_receipts.insert(
            &batch_id,
            &domain::RedeemStakeBatchReceipt::new((2 * YOCTO).into(), contract.stake_token_value),
        );

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;
        assert_eq!(account.near.unwrap().amount(), (YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());

        // Then there should be 1 STAKE left unclaimed on the receipt
        let receipt = contract.redeem_stake_batch_receipts.get(&batch_id).unwrap();
        assert_eq!(receipt.redeemed_stake(), YOCTO.into());
    }

    #[test]
    fn when_account_has_redeem_stake_batch_and_next_redeem_stake_batch_with_receipts_for_both() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        let batch_id_1 = {
            let mut account = contract.predecessor_registered_account();
            account.apply_stake_credit(YOCTO.into());
            contract.save_registered_account(&account);
            let batch_id = contract
                .redeem_all()
                .map(|batch_id| domain::BatchId(batch_id.into()))
                .unwrap();
            contract.redeem_stake_batch_receipts.insert(
                &batch_id,
                &domain::RedeemStakeBatchReceipt::new(
                    (2 * YOCTO).into(),
                    contract.stake_token_value,
                ),
            );
            batch_id
        };

        let batch_id_2 = {
            let mut account = contract.predecessor_registered_account();
            account.apply_stake_credit(YOCTO.into());
            contract.save_registered_account(&account);
            contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
            let batch_id = contract
                .redeem_all()
                .map(|batch_id| domain::BatchId(batch_id.into()))
                .unwrap();
            contract.redeem_stake_batch_receipts.insert(
                &batch_id,
                &domain::RedeemStakeBatchReceipt::new(
                    (4 * YOCTO).into(),
                    contract.stake_token_value,
                ),
            );
            contract.run_redeem_stake_batch_lock = None;
            batch_id
        };

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;
        assert_eq!(account.near.unwrap().amount(), (2 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());

        // Then there should be 1 STAKE left unclaimed on the receipt
        let receipt = contract
            .redeem_stake_batch_receipts
            .get(&batch_id_1)
            .unwrap();
        assert_eq!(receipt.redeemed_stake(), YOCTO.into());

        let receipt = contract
            .redeem_stake_batch_receipts
            .get(&batch_id_2)
            .unwrap();
        assert_eq!(receipt.redeemed_stake(), (3 * YOCTO).into());
    }

    #[test]
    fn when_account_has_redeem_stake_batch_and_next_redeem_stake_batch_with_receipt_for_both_fully_claimed(
    ) {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        let batch_id_1 = {
            let mut account = contract.predecessor_registered_account();
            account.apply_stake_credit(YOCTO.into());
            contract.save_registered_account(&account);
            let batch_id = contract
                .redeem_all()
                .map(|batch_id| domain::BatchId(batch_id.into()))
                .unwrap();
            contract.redeem_stake_batch_receipts.insert(
                &batch_id,
                &domain::RedeemStakeBatchReceipt::new(YOCTO.into(), contract.stake_token_value),
            );
            batch_id
        };

        let batch_id_2 = {
            let mut account = contract.predecessor_registered_account();
            account.apply_stake_credit(YOCTO.into());
            contract.save_registered_account(&account);
            contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
            let batch_id = contract
                .redeem_all()
                .map(|batch_id| domain::BatchId(batch_id.into()))
                .unwrap();
            contract.redeem_stake_batch_receipts.insert(
                &batch_id,
                &domain::RedeemStakeBatchReceipt::new(YOCTO.into(), contract.stake_token_value),
            );
            contract.run_redeem_stake_batch_lock = None;
            batch_id
        };

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;
        assert_eq!(account.near.unwrap().amount(), (2 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());

        // Then there should be 1 STAKE left unclaimed on the receipt
        assert!(contract
            .redeem_stake_batch_receipts
            .get(&batch_id_1)
            .is_none());
        assert!(contract
            .redeem_stake_batch_receipts
            .get(&batch_id_2)
            .is_none());
    }

    #[test]
    fn when_account_has_redeem_stake_batch_and_next_redeem_stake_batch_with_receipts_for_current() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        {
            let mut account = contract.predecessor_registered_account();
            account.apply_stake_credit(YOCTO.into());
            contract.save_registered_account(&account);
            let batch_id = contract
                .redeem_all()
                .map(|batch_id| domain::BatchId(batch_id.into()))
                .unwrap();
            contract.redeem_stake_batch_receipts.insert(
                &batch_id,
                &domain::RedeemStakeBatchReceipt::new(
                    (2 * YOCTO).into(),
                    contract.stake_token_value,
                ),
            );
            batch_id
        };

        let batch_id_2 = {
            let mut account = contract.predecessor_registered_account();
            account.apply_stake_credit(YOCTO.into());
            contract.save_registered_account(&account);
            contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
            let batch_id = contract
                .redeem_all()
                .map(|batch_id| domain::BatchId(batch_id.into()))
                .unwrap();
            contract.run_redeem_stake_batch_lock = None;
            batch_id
        };

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;
        assert_eq!(account.near.unwrap().amount(), YOCTO.into());
        assert_eq!(account.redeem_stake_batch.unwrap().id(), batch_id_2);
        assert!(account.next_redeem_stake_batch.is_none());
    }

    /// Given an account has redeemed STAKE
    /// And the batch receipt is pending withdrawal
    /// And there is enough NEAR liquidity to fulfill the claim
    /// Then the account can claim the NEAR funds from the NEAR liquidity pool
    #[test]
    fn when_account_claims_against_liquidity() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;
        let mut context = test_context.context.clone();

        let mut registered_account = contract.predecessor_registered_account();
        let account = &mut registered_account.account;
        account.apply_stake_credit(YOCTO.into());
        contract.save_registered_account(&registered_account);

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        let batch_id = contract
            .redeem_all()
            .map(|id| domain::BatchId(id.into()))
            .unwrap();

        contract.near_liquidity_pool = YOCTO.into();
        contract.redeem_stake_batch_receipts.insert(
            &batch_id,
            &domain::RedeemStakeBatchReceipt::new((2 * YOCTO).into(), contract.stake_token_value),
        );
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;
        assert!(account.stake.is_none());
        assert_eq!(account.near.unwrap().amount(), YOCTO.into());
        assert!(account.redeem_stake_batch.is_none());
        assert_eq!(contract.near_liquidity_pool, 0.into());
        assert_eq!(
            contract.pending_withdrawal().unwrap().redeemed_stake,
            YOCTO.into()
        );
    }

    /// Given an account has redeemed STAKE
    /// And the batch receipt is pending withdrawal
    /// And there is enough NEAR liquidity to fulfill the claim
    /// And the receipt is fully claimed
    /// Then the account can claim the NEAR funds from the NEAR liquidity pool
    /// And the RedeemLock is set to None
    /// And the receipt has been deleted
    #[test]
    fn when_account_claims_from_liquidity_pool_and_closes_out_pending_withdrawal() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;
        let mut context = test_context.context.clone();

        let mut registered_account = contract.predecessor_registered_account();
        let account = &mut registered_account.account;
        account.apply_stake_credit(YOCTO.into());
        contract.save_registered_account(&registered_account);

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        let batch_id = contract
            .redeem_all()
            .map(|id| domain::BatchId(id.into()))
            .unwrap();

        contract.near_liquidity_pool = YOCTO.into();
        contract.redeem_stake_batch_receipts.insert(
            &batch_id,
            &domain::RedeemStakeBatchReceipt::new(YOCTO.into(), contract.stake_token_value),
        );
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;
        assert!(account.stake.is_none());
        assert_eq!(account.near.unwrap().amount(), YOCTO.into());
        assert!(account.redeem_stake_batch.is_none());
        assert_eq!(contract.near_liquidity_pool, 0.into());
        assert!(contract.pending_withdrawal().is_none());
        assert!(contract.run_redeem_stake_batch_lock.is_none());
    }

    #[test]
    fn when_account_claims_from_liquidity_pool_and_liquidity_results_in_rounding_down_stake() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        let mut registered_account = contract.predecessor_registered_account();
        let account = &mut registered_account.account;
        account.apply_stake_credit(YOCTO.into());
        contract.save_registered_account(&registered_account);

        let batch_id = contract
            .redeem_all()
            .map(|id| domain::BatchId(id.into()))
            .unwrap();

        // contract has 1 NEAR in liquidity pool
        contract.near_liquidity_pool = YOCTO.into();
        // exchange rate is 1 STAKE -> 3 NEAR
        contract.redeem_stake_batch_receipts.insert(
            &batch_id,
            &domain::RedeemStakeBatchReceipt::new(
                YOCTO.into(),
                domain::StakeTokenValue::new(
                    BlockTimeHeight::from_env(),
                    (3 * YOCTO).into(),
                    YOCTO.into(),
                ),
            ),
        );
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;
        // account's STAKE balance should be zero because all STAKE was redeemed
        assert!(account.stake.is_none());

        assert_eq!(account.near.unwrap().amount(), YOCTO.into());
        assert_eq!(
            account.redeem_stake_batch.unwrap().balance().amount(),
            (YOCTO - (YOCTO / 3)).into()
        );
        assert_eq!(contract.near_liquidity_pool, 0.into());
        assert_eq!(
            contract.pending_withdrawal().unwrap().redeemed_stake,
            (YOCTO - (YOCTO / 3)).into()
        );
        assert!(contract.run_redeem_stake_batch_lock.is_some());

        // Arrange - unstaked NEAR has been withdrawn from staking pool
        contract.run_redeem_stake_batch_lock = None;

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account().account;
        assert_eq!(account.near.unwrap().amount(), (3 * YOCTO + 1).into());
        println!(
            "account.redeem_stake_batch: {:?}",
            account.redeem_stake_batch
        );
        assert!(account.redeem_stake_batch.is_none());
        println!(
            "contract.pending_withdrawal(): {:?}",
            contract.pending_withdrawal()
        );
        assert!(contract.pending_withdrawal().is_none());
    }

    /// Given an account has redeemed STAKE into the current and next batches
    /// And there is a receipt for the current batch
    /// When the account claims funds, the current batch funds will be claimed
    /// And the next batch gets moved into the current batch slot
    #[test]
    fn claim_redeem_stake_batch_receipts_for_current_and_next_batch_with_receipt_for_current() {
        let mut ctx = TestContext::with_registered_account();
        let contract = &mut ctx.contract;

        let mut account = contract.predecessor_registered_account();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        *contract.batch_id_sequence += 1;
        account.next_redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (15 * YOCTO).into(),
        ));
        contract.save_registered_account(&account);

        contract.redeem_stake_batch_receipts.insert(
            &(contract.batch_id_sequence.value() - 1).into(),
            &domain::RedeemStakeBatchReceipt::new((10 * YOCTO).into(), contract.stake_token_value),
        );

        contract.claim_receipt_funds(&mut account);
        contract.save_registered_account(&account);
        let account = contract.predecessor_registered_account();
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
        let mut ctx = TestContext::with_registered_account();
        let contract = &mut ctx.contract;

        let mut account = contract.predecessor_registered_account();
        let batch_id = contract.batch_id_sequence;
        account.redeem_stake_batch =
            Some(domain::RedeemStakeBatch::new(batch_id, (10 * YOCTO).into()));
        account.next_redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            (batch_id.value() + 1).into(),
            (10 * YOCTO).into(),
        ));
        contract.save_registered_account(&account);

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
        contract.save_registered_account(&account);
        let account = contract.predecessor_registered_account();
        assert_eq!(account.near.unwrap().amount(), (20 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());

        let receipt = contract.redeem_stake_batch_receipts.get(&batch_id).unwrap();
        assert_eq!(receipt.redeemed_stake(), (10 * YOCTO).into());
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::domain::BlockTimeHeight;
    use crate::near::UNSTAKED_NEAR_FUNDS_NUM_EPOCHS_TO_UNLOCK;
    use crate::{
        interface::{AccountManagement, Operator},
        near::YOCTO,
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
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();

        // Then the funds are deposited into the current stake batch on the account
        let account_id = test_context.account_id;
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
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;
        contract.run_stake_batch_locked = true;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();
        let account = contract
            .lookup_account(test_context.account_id.try_into().unwrap())
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
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();
        let account = contract
            .lookup_account(test_context.account_id.try_into().unwrap())
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
            .lookup_account(test_context.account_id.try_into().unwrap())
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
    fn claim_receipt_funds_with_no_batched_funds() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // should have no effect because there are no batches and no receipts
        let mut account = contract.registered_account(test_context.account_id);
        contract.claim_receipt_funds(&mut account);
    }

    /// Given the account has funds in the stake batch
    /// And there is no receipt for the batch
    /// When funds are claimed
    /// Then there should be no effect on the account
    #[test]
    fn claim_receipt_funds_with_funds_in_stake_batch_and_no_receipt() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let mut account = contract.registered_account(test_context.account_id);
        let batch_id = contract.deposit_near_for_account_to_stake(&mut account, YOCTO.into());
        contract.save_registered_account(&account);

        // When batch receipts are claimed
        contract.claim_receipt_funds(&mut account);
        contract.save_registered_account(&account);
        // Then there should be no effect on the account
        let account = contract
            .lookup_account(test_context.account_id.try_into().unwrap())
            .unwrap();
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(stake_batch.id, batch_id.into());
        assert_eq!(stake_batch.balance.amount, YOCTO.into());
    }

    /// Given the account has funds in the stake batch
    /// And there is a receipt for the batch with additional funds batched into it
    /// When funds are claimed
    /// Then the STAKE tokens should be credited to the account
    /// And the receipt NEAR balance should have been debited
    #[test]
    fn claim_receipt_funds_with_funds_in_stake_batch_and_with_receipt() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let mut account = contract.registered_account(test_context.account_id);
        let batch_id = contract.deposit_near_for_account_to_stake(&mut account, YOCTO.into());
        contract.save_registered_account(&account);
        let mut account = contract.registered_account(test_context.account_id);

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
        contract.save_registered_account(&account);
        // Assert
        let account = contract
            .lookup_account(test_context.account_id.try_into().unwrap())
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
        let mut account = contract.registered_account(test_context.account_id);
        let batch_id = contract.deposit_near_for_account_to_stake(&mut account, YOCTO.into());
        contract.save_registered_account(&account);
        // When batch receipts are claimed
        contract.claim_receipt_funds(&mut account);
        contract.save_registered_account(&account);
        // Assert
        let account = contract
            .lookup_account(test_context.account_id.try_into().unwrap())
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
        let batch_id = domain::BatchId(batch_id.value());
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
    fn claim_receipt_funds_with_all_stake_batch_funds_claimed_on_receipt() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let mut account = contract.registered_account(test_context.account_id);
        let batch_id = contract.deposit_near_for_account_to_stake(&mut account, (2 * YOCTO).into());
        contract.save_registered_account(&account);
        let mut account = contract.registered_account(test_context.account_id);

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
        contract.save_registered_account(&account);

        // Assert
        let account = contract
            .lookup_account(test_context.account_id.try_into().unwrap())
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
    fn claim_receipt_funds_with_stake_batch_and_next_stake_batch_funds_with_receipts() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let mut account = contract.registered_account(test_context.account_id);
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
        contract.save_registered_account(&account);

        let account = contract
            .lookup_account(test_context.account_id.try_into().unwrap())
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
        let mut account = contract.registered_account(test_context.account_id);
        contract.claim_receipt_funds(&mut account);
        contract.save_registered_account(&account);
        // then receipts should be deleted because all funds have been claimed
        assert!(contract
            .stake_batch_receipts
            .get(&domain::BatchId(stake_batch_id.into()))
            .is_none());

        let account = contract
            .lookup_account(test_context.account_id.try_into().unwrap())
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
    #[should_panic(expected = "ILLEGAL STATE : stake batch should exist")]
    fn stake_no_stake_batch() {
        let mut test_context = TestContext::with_registered_account();
        test_context.contract.stake();
    }

    /// Given the contract has a stake batch
    /// When the stake batch is run
    /// Then the contract is locked
    /// When the stake batch is run again while the contract is locked
    /// Then the func call panics
    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn stake_contract_when_stake_batch_in_progress() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.deposit();
        context.account_balance += context.attached_deposit;

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
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

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
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);
        contract.stake();
    }

    #[test]
    fn deposit_and_stake_contract_when_redeem_stake_batch_in_progress_unstaking() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);
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
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.stake();
    }

    #[test]
    fn deposit_and_stake_contract_when_redeem_status_pending_withdrawal() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        *contract.batch_id_sequence += 1;
        let redeem_stake_batch =
            domain::RedeemStakeBatch::new(contract.batch_id_sequence, YOCTO.into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);

        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.deposit_and_stake();
    }

    /// Given the contract has just been deployed
    /// And the STAKE token value is retrieved within the same epoch
    /// Then the cached version should be returned
    #[test]
    fn stake_token_value_is_current() {
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

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

    #[test]
    fn deposit_and_stake_success() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

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

        let receipts: Vec<Receipt> = deserialize_receipts();
        assert_eq!(receipts.len(), 3);

        {
            let receipt = &receipts[0];
            assert_eq!(receipt.actions.len(), 2);
            {
                let action = &receipt.actions[0];
                match action {
                    Action::FunctionCall { method_name, .. } => {
                        assert_eq!(method_name, "deposit_and_stake")
                    }
                    _ => panic!("expected `deposit_and_stake` func call on staking pool"),
                }
            }
            {
                let action = &receipt.actions[1];
                match action {
                    Action::FunctionCall { method_name, .. } => {
                        assert_eq!(method_name, "get_account")
                    }
                    _ => panic!("expected `get_account` func call on staking pool"),
                }
            }
        }

        {
            let receipt = &receipts[1];
            let action = &receipt.actions[0];
            match action {
                Action::FunctionCall { method_name, .. } => {
                    assert_eq!(method_name, "on_deposit_and_stake")
                }
                _ => panic!("expected `on_deposit_and_stake` func call on staking pool"),
            }
        }

        {
            let receipt = &receipts[2];
            let action = &receipt.actions[0];
            match action {
                Action::FunctionCall { method_name, .. } => {
                    assert_eq!(method_name, "release_run_stake_batch_lock")
                }
                _ => panic!("expected `on_deposit_and_stake` callback"),
            }
        }
    }

    /// Given the funds were successfully deposited and staked into the staking pool
    /// Then the stake batch receipts is saved
    /// And the total STAKE supply is updated
    /// And if there are funds in the next stake batch, then move it into the current batch
    #[test]
    fn stake_workflow_success() {
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        {
            let staked_near_amount = 100 * YOCTO;
            context.attached_deposit = staked_near_amount;
            testing_env!(context.clone());
            contract.deposit();
            context.account_balance += context.attached_deposit;

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
                        unstaked_balance: YOCTO.into(),
                        staked_balance: (99 * YOCTO).into(),
                        can_withdraw: true,
                    };
                    contract.on_run_stake_batch(staking_pool_account.clone()); // callback

                    {
                        context.predecessor_account_id = context.current_account_id.clone();
                        testing_env!(context.clone());
                        contract.on_deposit_and_stake(None, staking_pool_account); // callback

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
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;

        assert!(contract.redeem_stake_batch.is_none());
        assert!(contract.next_redeem_stake_batch.is_none());

        // Given the account has STAKE
        let mut account = contract.registered_account(test_context.account_id);
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        let initial_account_stake = (50 * YOCTO).into();
        account.apply_stake_credit(initial_account_stake);
        contract.save_registered_account(&account);

        let redeem_amount = YoctoStake::from(10 * YOCTO);
        let batch_id = contract.redeem(redeem_amount.clone());

        let batch = contract
            .redeem_stake_batch
            .expect("current stake batch should have funds");
        assert_eq!(batch_id, batch.id().into());
        assert_eq!(redeem_amount, batch.balance().amount().into());

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
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
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
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
        let mut test_context = TestContext::with_registered_account();
        let contract = &mut test_context.contract;
        assert!(contract.redeem_stake_batch.is_none());
        assert!(contract.next_redeem_stake_batch.is_none());

        // Given the account has STAKE
        let mut account = contract.registered_account(test_context.account_id);
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        let initial_account_stake = (50 * YOCTO).into();
        account.apply_stake_credit(initial_account_stake);
        contract.save_registered_account(&account);

        let redeem_amount = YoctoStake::from(10 * YOCTO);
        let batch_id = contract.redeem(redeem_amount.clone());

        let batch = contract
            .redeem_stake_batch
            .expect("current stake batch should have funds");
        assert_eq!(batch_id, batch.id().into());
        assert_eq!(redeem_amount, batch.balance().amount().into());

        let account = contract
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
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
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
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
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;
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

        let account = contract.registered_account(test_context.account_id);
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
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;
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

        let account = contract.registered_account(test_context.account_id);
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
        let mut test_context = TestContext::with_registered_account();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        assert!(contract.redeem_stake_batch.is_none());
        assert!(contract.next_redeem_stake_batch.is_none());

        // Given the account has STAKE
        let mut account = contract.registered_account(test_context.account_id);
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        let initial_account_stake = (50 * YOCTO).into();
        account.apply_stake_credit(initial_account_stake);
        contract.save_registered_account(&account);

        let batch_id = contract.redeem_all().unwrap();
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
            .lookup_account(ValidAccountId::try_from(test_context.account_id).unwrap())
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
    #[allow(dead_code)]
    struct GetStakedAccountBalanceArgs {
        account_id: String,
    }

    /// Given the contract is unlocked and has no batch runs in progress
    /// And there is a redeem stake batch
    /// When the redeem batch is run
    /// Then it creates the following receipts
    ///   - func call to get account from staking pool
    ///   - func call for callback to clear the release lock if the state is `Unstaking`
    // #[test]
    // fn unstake_no_locks() {
    //     let account_id = "alfio-zappala.near";
    //     let mut context = new_context(account_id);
    //     context.attached_deposit = YOCTO;
    //     context.account_balance = 100 * YOCTO;
    //     testing_env!(context.clone());
    //
    //     let contract_settings = default_contract_settings();
    //     let mut contract = StakeTokenContract::new(None, contract_settings.clone());
    //
    //     *contract.batch_id_sequence += 1;
    //     contract.redeem_stake_batch = Some(RedeemStakeBatch::new(
    //         contract.batch_id_sequence,
    //         (10 * YOCTO).into(),
    //     ));
    //
    //     contract.unstake();
    //     assert_eq!(
    //         contract.run_redeem_stake_batch_lock,
    //         Some(RedeemLock::Unstaking)
    //     );
    //     let receipts = deserialize_receipts(&env::created_receipts());
    //     println!("receipt count = {}\n{:#?}", receipts.len(), receipts);
    //     assert_eq!(receipts.len(), 3);
    //     let receipts = receipts.as_slice();
    //     {
    //         let receipt = receipts.first().unwrap();
    //         assert_eq!(receipt.receiver_id, contract.staking_pool_id);
    //
    //         let actions = receipt.actions.as_slice();
    //         let func_call_action = actions.first().unwrap();
    //         match func_call_action {
    //             Action::FunctionCall {
    //                 method_name, args, ..
    //             } => {
    //                 assert_eq!(method_name, "get_account");
    //                 let args: GetStakedAccountBalanceArgs =
    //                     near_sdk::serde_json::from_str(args).unwrap();
    //                 assert_eq!(args.account_id, context.current_account_id);
    //             }
    //             _ => panic!("expected func call action"),
    //         }
    //     }
    //     {
    //         let receipt = &receipts[1];
    //         assert_eq!(receipt.receiver_id, env::current_account_id());
    //
    //         let actions = receipt.actions.as_slice();
    //         let func_call_action = actions.first().unwrap();
    //         match func_call_action {
    //             Action::FunctionCall {
    //                 method_name, args, ..
    //             } => {
    //                 assert_eq!(method_name, "on_run_redeem_stake_batch");
    //                 assert!(args.is_empty());
    //             }
    //             _ => panic!("expected func call action"),
    //         }
    //     }
    //     {
    //         let receipt = &receipts[2];
    //         assert_eq!(receipt.receiver_id, env::current_account_id());
    //
    //         let actions = receipt.actions.as_slice();
    //         let func_call_action = actions.first().unwrap();
    //         match func_call_action {
    //             Action::FunctionCall {
    //                 method_name, args, ..
    //             } => {
    //                 assert_eq!(method_name, "release_run_redeem_stake_batch_unstaking_lock");
    //                 assert!(args.is_empty());
    //             }
    //             _ => panic!("expected func call action"),
    //         }
    //     }
    // }

    // #[test]
    // fn redeem_and_unstake_no_locks() {
    //     let account_id = "alfio-zappala.near";
    //     let mut context = new_context(account_id);
    //     context.attached_deposit = YOCTO;
    //     context.account_balance = 100 * YOCTO;
    //     testing_env!(context.clone());
    //
    //     let contract_settings = default_contract_settings();
    //     let mut contract = StakeTokenContract::new(None, contract_settings.clone());
    //
    //     testing_env!(context.clone());
    //     contract.register_account();
    //     let mut account = contract.registered_account(account_id);
    //     account.stake = Some(TimestampedStakeBalance::new((100 * YOCTO).into()));
    //     contract.save_registered_account(&account);
    //
    //     testing_env!(context.clone());
    //     contract.redeem_and_unstake((10 * YOCTO).into());
    //
    //     assert_eq!(
    //         contract.run_redeem_stake_batch_lock,
    //         Some(RedeemLock::Unstaking)
    //     );
    //     let receipts = deserialize_receipts(&env::created_receipts());
    //     println!("receipt count = {}\n{:#?}", receipts.len(), receipts);
    //     assert_eq!(receipts.len(), 3);
    //     let receipts = receipts.as_slice();
    //     {
    //         let receipt = receipts.first().unwrap();
    //         assert_eq!(receipt.receiver_id, contract.staking_pool_id);
    //
    //         let actions = receipt.actions.as_slice();
    //         let func_call_action = actions.first().unwrap();
    //         match func_call_action {
    //             Action::FunctionCall {
    //                 method_name, args, ..
    //             } => {
    //                 assert_eq!(method_name, "get_account");
    //                 let args: GetStakedAccountBalanceArgs =
    //                     near_sdk::serde_json::from_str(args).unwrap();
    //                 assert_eq!(args.account_id, context.current_account_id);
    //             }
    //             _ => panic!("expected func call action"),
    //         }
    //     }
    //     {
    //         let receipt = &receipts[1];
    //         assert_eq!(receipt.receiver_id, env::current_account_id());
    //
    //         let actions = receipt.actions.as_slice();
    //         let func_call_action = actions.first().unwrap();
    //         match func_call_action {
    //             Action::FunctionCall {
    //                 method_name, args, ..
    //             } => {
    //                 assert_eq!(method_name, "on_run_redeem_stake_batch");
    //                 assert!(args.is_empty());
    //             }
    //             _ => panic!("expected func call action"),
    //         }
    //     }
    //     {
    //         let receipt = &receipts[2];
    //         assert_eq!(receipt.receiver_id, env::current_account_id());
    //
    //         let actions = receipt.actions.as_slice();
    //         let func_call_action = actions.first().unwrap();
    //         match func_call_action {
    //             Action::FunctionCall {
    //                 method_name, args, ..
    //             } => {
    //                 assert_eq!(method_name, "release_run_redeem_stake_batch_unstaking_lock");
    //                 assert!(args.is_empty());
    //             }
    //             _ => panic!("expected func call action"),
    //         }
    //     }
    // }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn unstake_locked_for_staking() {
        // Arrange
        let mut context = TestContext::with_registered_account();
        let contract = &mut context.contract;
        contract.run_stake_batch_locked = true;

        // Act
        contract.unstake();
    }

    #[test]
    fn redeem_and_unstake_locked_for_staking() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        contract.run_stake_batch_locked = true;
        let mut account = contract.predecessor_registered_account();
        account.stake = Some(TimestampedStakeBalance::new((100 * YOCTO).into()));
        contract.save_registered_account(&account);

        if let PromiseOrValue::Value(batch_id) = contract.redeem_and_unstake((10 * YOCTO).into()) {
            assert_eq!(batch_id, contract.redeem_stake_batch.unwrap().id().into());
        } else {
            panic!("expected batch ID to be returned because unstake workflow cannot be run if a batch is running");
        }
    }

    #[test]
    #[should_panic(expected = "action is blocked because a batch is running")]
    fn unstake_locked_for_unstaking() {
        // Arrange
        let mut context = TestContext::with_registered_account();
        let contract = &mut context.contract;
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        // Act
        contract.unstake();
    }

    #[test]
    fn redeem_and_unstake_locked_for_unstaking() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        let mut account = contract.predecessor_registered_account();
        account.stake = Some(TimestampedStakeBalance::new((100 * YOCTO).into()));
        contract.save_registered_account(&account);

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
        let mut contract = TestContext::with_registered_account().contract;
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
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;
        let mut context = test_ctx.context.clone();

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
        let receipts = deserialize_receipts();
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

    /// Given an account has redeemed STAKE
    /// And the batch has completed
    /// Then the account can claim the NEAR funds
    #[test]
    fn claim_receipt_funds_on_reddeem_stake_batch_receipt() {
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let mut account = contract.predecessor_registered_account();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new((20 * YOCTO).into(), contract.stake_token_value),
        );

        contract.claim_receipt_funds(&mut account);
        contract.save_registered_account(&account);
        let account = contract.predecessor_registered_account();
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
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let mut account = contract.predecessor_registered_account();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        *contract.batch_id_sequence += 1;
        account.next_redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (15 * YOCTO).into(),
        ));
        contract.save_registered_account(&account);

        contract.redeem_stake_batch_receipts.insert(
            &(contract.batch_id_sequence.value() - 1).into(),
            &domain::RedeemStakeBatchReceipt::new((10 * YOCTO).into(), contract.stake_token_value),
        );
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new((20 * YOCTO).into(), contract.stake_token_value),
        );

        contract.claim_receipt_funds(&mut account);
        contract.save_registered_account(&account);
        let account = contract.predecessor_registered_account();
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
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let mut account = contract.predecessor_registered_account();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.save_registered_account(&account);

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
        contract.save_registered_account(&account);
        let account = contract.predecessor_registered_account();
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
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let mut account = contract.predecessor_registered_account();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.save_registered_account(&account);

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

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account();
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
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        // account has redeemed 10 STAKE in current batch and 15 STAKE in next batch
        let mut account = contract.predecessor_registered_account();
        account.redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        //contract has receipt that matches exact value of account's batch amount
        contract.redeem_stake_batch_receipts.insert(
            &(contract.batch_id_sequence.value()).into(),
            &domain::RedeemStakeBatchReceipt::new(
                account.redeem_stake_batch.unwrap().balance().amount(),
                contract.stake_token_value,
            ),
        );
        *contract.batch_id_sequence += 1;
        account.next_redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (15 * YOCTO).into(),
        ));
        contract.save_registered_account(&account);

        // Act
        contract.claim_receipts();

        // Assert
        let account = contract.predecessor_registered_account();
        assert_eq!(account.near.unwrap().amount(), (10 * YOCTO).into());
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
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let mut account = contract.predecessor_registered_account();
        let batch_id = contract.batch_id_sequence;
        account.redeem_stake_batch =
            Some(domain::RedeemStakeBatch::new(batch_id, (10 * YOCTO).into()));
        account.next_redeem_stake_batch = Some(domain::RedeemStakeBatch::new(
            (batch_id.value() + 1).into(),
            (10 * YOCTO).into(),
        ));
        contract.save_registered_account(&account);

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
        contract.save_registered_account(&account);
        let account = contract.predecessor_registered_account();
        assert_eq!(account.near.unwrap().amount(), (20 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());

        let receipt = contract.redeem_stake_batch_receipts.get(&batch_id).unwrap();
        assert_eq!(receipt.redeemed_stake(), (10 * YOCTO).into());
    }

    #[test]
    fn apply_unclaimed_receipts_to_account() {
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;
        let mut context = test_ctx.context.clone();

        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        let mut account = contract.predecessor_registered_account();

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
        contract.save_registered_account(&account);

        context.is_view = true;
        testing_env!(context.clone());
        let account = contract
            .lookup_account(to_valid_account_id(TEST_ACCOUNT_ID))
            .unwrap();
        assert!(account.stake_batch.is_none());
        assert!(account.redeem_stake_batch.is_none());
        assert!(account.next_stake_batch.is_none());
        assert!(account.next_redeem_stake_batch.is_none());
        assert_eq!(account.stake.unwrap().amount, (2 * 10 * YOCTO).into());
        assert_eq!(account.near.unwrap().amount, (2 * 2 * YOCTO).into());
    }

    #[test]
    fn cancel_pending_redeem_stake_request_success() {
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let mut account = contract.predecessor_registered_account();
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);

        contract.redeem((10 * YOCTO).into());

        let account = contract.predecessor_registered_account();
        assert_eq!(account.stake.unwrap().amount(), (90 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_some());
        assert!(contract.redeem_stake_batch.is_some());

        assert_eq!(
            contract.remove_all_from_redeem_stake_batch(),
            (10 * YOCTO).into()
        );
        let account = contract.predecessor_registered_account();
        assert_eq!(account.stake.unwrap().amount(), (100 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());
        assert!(contract.redeem_stake_batch.is_none());
    }

    #[test]
    fn cancel_pending_redeem_stake_request_success_with_funds_remaining_in_batch() {
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let mut account = contract.predecessor_registered_account();
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);

        contract.redeem((10 * YOCTO).into());
        {
            let mut batch = contract.redeem_stake_batch.unwrap();
            batch.add(YOCTO.into());
            contract.redeem_stake_batch = Some(batch);
        }

        let account = contract.predecessor_registered_account();
        assert_eq!(account.stake.unwrap().amount(), (90 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_some());
        assert!(contract.redeem_stake_batch.is_some());

        assert_eq!(
            contract.remove_all_from_redeem_stake_batch(),
            (10 * YOCTO).into()
        );
        let account = contract.predecessor_registered_account();
        assert_eq!(account.stake.unwrap().amount(), (100 * YOCTO).into());
        assert!(account.redeem_stake_batch.is_none());
        assert_eq!(
            contract.redeem_stake_batch.unwrap().balance().amount(),
            YOCTO.into()
        );
    }

    #[test]
    fn cancel_pending_redeem_stake_request_while_locked_success() {
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let mut account = contract.predecessor_registered_account();
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);

        contract.redeem((10 * YOCTO).into());

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.redeem((10 * YOCTO).into());

        let account = contract.predecessor_registered_account();
        assert_eq!(account.stake.unwrap().amount(), (80 * YOCTO).into());
        assert!(account.next_redeem_stake_batch.is_some());
        assert!(contract.next_redeem_stake_batch.is_some());

        assert_eq!(
            contract.remove_all_from_redeem_stake_batch(),
            (10 * YOCTO).into()
        );
        let account = contract.predecessor_registered_account();
        assert_eq!(account.stake.unwrap().amount(), (90 * YOCTO).into());
        assert!(account.next_redeem_stake_batch.is_none());
        assert!(contract.next_redeem_stake_batch.is_none());
    }

    #[test]
    fn cancel_pending_redeem_stake_request_while_locked_success_with_other_funds_in_batch() {
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let mut account = contract.predecessor_registered_account();
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);

        contract.redeem((10 * YOCTO).into());

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.redeem((10 * YOCTO).into());
        {
            let mut batch = contract.next_redeem_stake_batch.unwrap();
            batch.add(YOCTO.into());
            contract.next_redeem_stake_batch = Some(batch);
        }

        let account = contract.predecessor_registered_account();
        assert_eq!(account.stake.unwrap().amount(), (80 * YOCTO).into());
        assert!(account.next_redeem_stake_batch.is_some());
        assert!(contract.next_redeem_stake_batch.is_some());

        assert_eq!(
            contract.remove_all_from_redeem_stake_batch(),
            (10 * YOCTO).into()
        );
        let account = contract.predecessor_registered_account();
        assert_eq!(account.stake.unwrap().amount(), (90 * YOCTO).into());
        assert!(account.next_redeem_stake_batch.is_none());
        assert_eq!(
            contract.next_redeem_stake_batch.unwrap().balance().amount(),
            YOCTO.into()
        );
    }

    #[test]
    fn cancel_pending_redeem_stake_request_no_batches_success() {
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        assert_eq!(contract.remove_all_from_redeem_stake_batch(), 0.into());
    }

    #[test]
    fn cancel_pending_redeem_stake_request_while_locked_no_next_batch_success() {
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);
        assert_eq!(contract.remove_all_from_redeem_stake_batch(), 0.into());
    }

    #[test]
    fn stake_batch_receipt_lookups() {
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

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
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

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

    #[test]
    fn stake_token_value_compensation() {
        // StakeTokenValue {
        //     total_staked_near_balance: 18503502971096472900569337,
        //     total_stake_supply: 18004621608054163628202638,
        //     stake_value: 1027708516952066370722278,
        //     block_height: 30530205,
        //     block_timestamp: 1609529212770398556,
        //     epoch_height: 128,
        // }

        // StakeTokenValue {
        //     total_staked_near_balance: 13364960386336141046957933,
        //     total_stake_supply: 13004621608054163628202638,
        //     stake_value: 1027708516952066370722277,
        //     block_height: 30530458,
        //     block_timestamp: 1609529367402036318,
        //     epoch_height: 128,
        // },

        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        contract.total_stake = TimestampedStakeBalance::new(18004621608054163628202638.into());
        contract.stake_token_value = StakeTokenValue::new(
            BlockTimeHeight::from_env(),
            18503502971096472900569337.into(),
            contract.total_stake.amount(),
        );
        let old_stake_token_value = contract.stake_token_value;

        contract.total_stake = TimestampedStakeBalance::new(13004621608054163628202638.into());
        contract.update_stake_token_value(13364960386336141046957933.into());
        let new_stake_token_value = contract.stake_token_value;
        println!(
            "current_stake_token_value: {:?} {:?}",
            old_stake_token_value.total_staked_near_balance(),
            old_stake_token_value.total_stake_supply()
        );

        println!(
            "new_stake_token_value: {:?} {:?}",
            new_stake_token_value.total_staked_near_balance(),
            new_stake_token_value.total_stake_supply()
        );
        println!(
            "compensation = {}",
            new_stake_token_value.total_staked_near_balance().value() - 13364960386336141046957933
        );
        println!(
            "{}\n{}",
            old_stake_token_value.stake_to_near(YOCTO.into()),
            new_stake_token_value.stake_to_near(YOCTO.into())
        );
        assert_eq!(
            old_stake_token_value.stake_to_near(YOCTO.into()),
            new_stake_token_value.stake_to_near(YOCTO.into())
        );
    }
}
