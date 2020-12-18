use crate::{
    core::Hash,
    domain::{self, Account, RedeemLock, RedeemStakeBatch, StakeBatch},
    interface::{
        BatchId, RedeemStakeBatchReceipt, StakeTokenValue, StakingService, YoctoNear, YoctoStake,
    },
    near::NO_DEPOSIT,
    StakeTokenContract,
};
use near_sdk::{
    env, ext_contract,
    json_types::U128,
    near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId, Promise,
};

#[near_bindgen]
impl StakingService for StakeTokenContract {
    fn staking_pool_id(&self) -> AccountId {
        self.staking_pool_id.clone()
    }

    fn deposit(&mut self) -> BatchId {
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = self.registered_account(&account_hash);

        let batch_id =
            self.deposit_near_for_account_to_stake(&mut account, env::attached_deposit().into());
        self.save_account(&account_hash, &account);
        batch_id
    }

    fn withdraw_funds_from_stake_batch(&mut self, _amount: YoctoNear) {
        unimplemented!()
    }

    fn withdraw_all_funds_from_stake_batch(&mut self) {
        unimplemented!()
    }

    /// logical workflow:
    /// 1. lock the contract
    /// 2. get account stake balance
    /// 3. deposit and stake NEAR funds
    /// 4. create stake batch receipt
    /// 5. update STAKE token supply
    /// 6. unlock contract
    fn run_stake_batch(&mut self) -> Promise {
        assert!(
            !self.run_stake_batch_locked,
            "staking batch is already in progress"
        );
        assert!(
            !self.is_unstaking(),
            "staking is blocked while unstaking is in progress"
        );
        assert!(
            self.stake_batch.is_some(),
            "there is no staking batch to run"
        );

        self.run_stake_batch_locked = true;

        self.get_account_staked_balance_from_staking_pool()
            .then(self.invoke_on_run_stake_batch())
            .then(self.invoke_release_run_stake_batch_lock())
    }

    fn redeem(&mut self, amount: YoctoStake) -> BatchId {
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = self.registered_account(&account_hash);
        let batch_id = self.redeem_stake_for_account(&mut account, amount.into());
        self.save_account(&account_hash, &account);
        batch_id
    }

    fn redeem_all(&mut self) -> BatchId {
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = self.registered_account(&account_hash);
        let amount = account.stake.expect("account has no stake").amount();
        let batch_id = self.redeem_stake_for_account(&mut account, amount);
        self.save_account(&account_hash, &account);
        batch_id
    }

    fn cancel_pending_redeem_stake_request(&mut self) -> bool {
        unimplemented!()
    }

    fn run_redeem_stake_batch(&mut self) -> Promise {
        assert!(
            !self.run_stake_batch_locked,
            "batch cannot be run while NEAR is being staked"
        );

        match self.run_redeem_stake_batch_lock {
            Some(RedeemLock::Unstaking) => panic!("batch is already in progress"),
            None => {
                assert!(
                    self.redeem_stake_batch.is_some(),
                    "there is no redeem stake batch"
                );
                self.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

                self.get_account_from_staking_pool()
                    .then(self.invoke_on_run_redeem_stake_batch())
                    .then(self.invoke_release_run_redeem_stake_batch_unstaking_lock())
            }
            Some(RedeemLock::PendingWithdrawal) => {
                let batch = self
                    .redeem_stake_batch
                    .expect("illegal state - batch does not exist");
                let batch_id = batch.id();
                let batch_receipt = self
                    .redeem_stake_batch_receipts
                    .get(&batch_id)
                    .expect("illegal state - batch receipt does not exist");
                assert!(
                    batch_receipt.unstaked_funds_available_for_withdrawal(),
                    "unstaked funds are not yet available for withdrawal"
                );

                self.get_account_from_staking_pool()
                    .then(self.invoke_on_redeeming_stake_pending_withdrawal())
            }
        }
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

    fn stake_token_value(&self) -> StakeTokenValue {
        self.stake_token_value.into()
    }

    fn refresh_stake_token_value(&self) -> Promise {
        self.get_account_staked_balance_from_staking_pool().then(
            ext_staking_pool_callbacks::on_refresh_account_staked_balance(
                &env::current_account_id(),
                NO_DEPOSIT.into(),
                self.config
                    .gas_config()
                    .callbacks()
                    .on_get_account_staked_balance()
                    .value(),
            ),
        )
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

    pub(crate) fn get_account_staked_balance_from_staking_pool(&self) -> Promise {
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
    }
}

impl StakeTokenContract {
    /// batches the NEAR to stake at the contract level and account level
    ///
    /// ## Panics
    /// if [amount] is zero
    ///
    /// ## Notes
    /// - before applying the deposit, batch receipts are processed [claim_receipt_funds]
    fn deposit_near_for_account_to_stake(
        &mut self,
        account: &mut Account,
        amount: domain::YoctoNear,
    ) -> BatchId {
        assert!(amount.value() > 0, "deposit is required in order to stake");

        self.claim_receipt_funds(account);

        // use current batch if not staking, i.e., the stake batch is not running
        if !self.run_stake_batch_locked {
            // apply at contract level
            let mut contract_batch = self.stake_batch.unwrap_or_else(|| {
                *self.batch_id_sequence += 1;
                StakeBatch::new(self.batch_id_sequence, domain::YoctoNear(0))
            });
            contract_batch.add(amount);
            self.stake_batch = Some(contract_batch);

            // apply at account level
            // NOTE: account batch ID must match contract batch ID
            let mut account_batch = account
                .stake_batch
                .unwrap_or_else(|| StakeBatch::new(contract_batch.id(), domain::YoctoNear(0)));
            account_batch.add(amount);
            account.stake_batch = Some(account_batch);

            account_batch.id().into()
        } else {
            // apply at contract level
            let mut contract_batch = self.next_stake_batch.unwrap_or_else(|| {
                *self.batch_id_sequence += 1;
                StakeBatch::new(self.batch_id_sequence, domain::YoctoNear(0))
            });
            contract_batch.add(amount);
            self.next_stake_batch = Some(contract_batch);

            // apply at account level
            // NOTE: account batch ID must match contract batch ID
            let mut account_batch = account
                .next_stake_batch
                .unwrap_or_else(|| StakeBatch::new(contract_batch.id(), domain::YoctoNear(0)));
            account_batch.add(amount);
            account.next_stake_batch = Some(account_batch);

            account_batch.id().into()
        }
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
        assert_ne!(amount.value(), 0, "redeem amount must not be zero");

        assert!(
            account
                .stake
                .map_or(false, |stake| stake.amount() >= amount),
            "account STAKE balance is insufficient to fulfill request"
        );

        // debit the amount of STAKE to redeem from the account
        let mut stake = account.stake.expect("account has zero STAKE token balance");
        stake.debit(amount);
        if stake.amount().value() > 0 {
            account.stake = Some(stake);
        } else {
            account.stake = None;
        }

        self.claim_receipt_funds(account);

        match self.run_redeem_stake_batch_lock {
            None => {
                // apply at contract level
                let mut contract_batch = self.redeem_stake_batch.unwrap_or_else(|| {
                    *self.batch_id_sequence += 1;
                    domain::RedeemStakeBatch::new(self.batch_id_sequence, domain::YoctoStake(0))
                });
                contract_batch.add(amount);
                self.redeem_stake_batch = Some(contract_batch);

                // apply at account level
                // NOTE: account batch ID must match contract batch ID
                let mut account_batch = account.redeem_stake_batch.unwrap_or_else(|| {
                    RedeemStakeBatch::new(contract_batch.id(), domain::YoctoStake(0))
                });
                account_batch.add(amount);
                account.redeem_stake_batch = Some(account_batch);

                account_batch.id().into()
            }
            Some(_redeem_lock) => {
                // apply at contract level
                let mut contract_batch = self.next_redeem_stake_batch.unwrap_or_else(|| {
                    *self.batch_id_sequence += 1;
                    domain::RedeemStakeBatch::new(self.batch_id_sequence, domain::YoctoStake(0))
                });
                contract_batch.add(amount);
                self.next_redeem_stake_batch = Some(contract_batch);

                // apply at account level
                // NOTE: account batch ID must match contract batch ID
                let mut account_batch = account.next_redeem_stake_batch.unwrap_or_else(|| {
                    RedeemStakeBatch::new(contract_batch.id(), domain::YoctoStake(0))
                });
                account_batch.add(amount);
                account.next_redeem_stake_batch = Some(account_batch);

                account_batch.id().into()
            }
        }
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

        let mut claimed_funds = false;

        match self.run_redeem_stake_batch_lock {
            // we can try to redeem receipts from previous batches
            // NOTE: batch IDs are sequential
            Some(RedeemLock::PendingWithdrawal) => {
                let pending_batch_id = self.redeem_stake_batch.expect("illegal state - if redeem lock is pending withdrawal, then there must be a batch").id();

                if let Some(batch) = account.redeem_stake_batch {
                    if batch.id().value() < pending_batch_id.value() {
                        if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                            claim_redeemed_stake_for_batch(self, account, batch, receipt);
                            account.redeem_stake_batch = None;
                            claimed_funds = true;
                        }
                    }
                }

                if let Some(batch) = account.next_redeem_stake_batch {
                    if batch.id().value() < pending_batch_id.value() {
                        if let Some(receipt) = self.redeem_stake_batch_receipts.get(&batch.id()) {
                            claim_redeemed_stake_for_batch(self, account, batch, receipt);
                            account.next_redeem_stake_batch = None;
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

        claimed_funds
    }

    fn is_unstaking(&self) -> bool {
        match self.run_redeem_stake_batch_lock {
            Some(RedeemLock::Unstaking) => true,
            _ => false,
        }
    }
}

type Balance = U128;

#[derive(Serialize, Deserialize)]
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

    fn deposit_and_stake(&mut self);

    fn unstake(&mut self, amount: Balance);

    fn get_account_staked_balance(&self, account_id: AccountId) -> Balance;

    fn get_account_unstaked_balance(&self, account_id: AccountId) -> Balance;

    fn is_account_unstaked_balance_available(&self, account_id: AccountId) -> bool;

    fn withdraw_all(&mut self);
}

#[ext_contract(ext_staking_pool_callbacks)]
pub trait ExtStakingPoolCallbacks {
    fn on_refresh_account_staked_balance(
        &mut self,
        #[callback] staked_balance: Balance,
    ) -> StakeTokenValue;
}

#[ext_contract(ext_redeeming_workflow_callbacks)]
pub trait ExtRedeemingWokflowCallbacks {
    fn on_run_redeem_stake_batch(
        &mut self,
        #[callback] staking_pool_account: StakingPoolAccount,
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
}

#[ext_contract(ext_staking_workflow_callbacks)]
pub trait ExtStakingWokflowCallbacks {
    /// callback for getting staked balance from staking pool as part of stake batch processing workflow
    ///
    /// ## Success Workflow
    /// 1. update the stake token value
    /// 2. deposit and stake funds with staking pool
    /// 3. register [on_deposit_and_stake] callback on the deposit and stake action
    fn on_run_stake_batch(&mut self, #[callback] staked_balance: Balance) -> Promise;

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

    use crate::interface::AccountManagement;
    use crate::near::{UNSTAKED_NEAR_FUNDS_NUM_EPOCHS_TO_UNLOCK, YOCTO};
    use crate::test_utils::*;
    use near_sdk::json_types::ValidAccountId;
    use near_sdk::{testing_env, MockedBlockchain};
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
        assert_eq!(stake_batch.balance.amount.value(), context.attached_deposit);
        assert_eq!(stake_batch.id, batch_id);
        assert!(account.next_stake_batch.is_none());

        // And the funds are deposited into the current stake batch on the contract
        assert_eq!(
            contract.stake_batch.unwrap().balance().amount(),
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
        let batch_id = contract.deposit_near_for_account_to_stake(&mut account, YOCTO.into());
        contract.save_account(&account_hash, &account);

        // When batch receipts are claimed
        contract.claim_all_batch_receipt_funds();
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
        let mut contract = StakeTokenContract::new(contract_settings);

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
        contract.claim_all_batch_receipt_funds();
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
        let mut contract = StakeTokenContract::new(contract_settings);

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
        let mut contract = StakeTokenContract::new(contract_settings);

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
    #[should_panic(expected = "there is no staking batch to run")]
    fn run_stake_batch_no_stake_batch() {
        let account_id = "alfio-zappala.near";
        let context = new_context(account_id);
        testing_env!(context);

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.run_stake_batch();
    }

    /// Given the contract has a stake batch
    /// When the stake batch is run
    /// Then the contract is locked
    /// When the stake batch is run again while the contract is locked
    /// Then the func call panics
    #[test]
    #[should_panic(expected = "staking batch is already in progress")]
    fn run_stake_batch_contract_when_stake_batch_in_progress() {
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

        context.attached_deposit = 0;
        testing_env!(context.clone());
        contract.run_stake_batch();
        assert!(contract.run_stake_batch_locked);

        testing_env!(context.clone());
        // should panic because contract is locked
        contract.run_stake_batch();
    }

    /// Given the contract is running the redeem stake batch
    /// When the stake batch is run
    /// Then the func call panics
    #[test]
    #[should_panic(expected = "staking is blocked while unstaking is in progress")]
    fn run_stake_batch_contract_when_redeem_stake_batch_in_progress_unstaking() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);

        contract.register_account();
        contract.run_stake_batch();
    }

    /// Given the contract is redeem status is pending withdrawal
    /// Then it is allowed to run stake batches
    #[test]
    fn run_stake_batch_contract_when_redeem_status_pending_withdrawal() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.register_account();

        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
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

        assert_eq!(
            contract.stake_token_value().total_stake_supply,
            contract.total_stake.amount().into()
        );
        assert_eq!(
            contract.stake_token_value().total_staked_near_balance,
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

        context.prepaid_gas = 10u64.pow(18);
        testing_env!(context.clone());
        contract.run_stake_batch();
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
                        method_name == "get_account_staked_balance"
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
        let mut contract = StakeTokenContract::new(contract_settings.clone());

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
        let mut contract = StakeTokenContract::new(contract_settings.clone());

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
        let mut contract = StakeTokenContract::new(contract_settings.clone());

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

    /// Given the contract is unlocked and has no batch runs in progress
    /// And there is a redeem stake batch
    /// When the redeem batch is run
    /// Then it creates the following receipts
    ///   - func call to get account from staking pool
    ///   - func call for callback to clear the release lock if the state is `Unstaking`
    #[test]
    fn run_redeem_batch_no_locks() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings.clone());

        *contract.batch_id_sequence += 1;
        contract.redeem_stake_batch = Some(RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));

        contract.run_redeem_stake_batch();
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
    #[should_panic(expected = "batch cannot be run while NEAR is being staked")]
    fn run_redeem_stake_batch_locked_for_staking() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings.clone());

        contract.run_stake_batch_locked = true;
        contract.run_redeem_stake_batch();
    }

    #[test]
    #[should_panic(expected = "batch is already in progress")]
    fn run_redeem_stake_batch_locked_for_unstaking() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings.clone());

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::Unstaking);
        contract.run_redeem_stake_batch();
    }

    #[test]
    #[should_panic(expected = "there is no redeem stake batch")]
    fn run_redeem_stake_batch_no_batch() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings.clone());

        contract.run_redeem_stake_batch();
    }

    /// Given the contract is unlocked and has no batch runs in progress
    /// And there is a redeem stake batch
    /// When the redeem batch is run
    /// Then it creates the following receipts
    ///   - func call to get account from staking pool
    ///   - func call for callback to clear the release lock if the state is `Unstaking`
    #[test]
    fn run_redeem_batch_pending_withdrawal() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings.clone());

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
        contract.run_redeem_stake_batch();
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
    #[should_panic(expected = "illegal state - batch does not exist")]
    fn run_redeem_batch_pending_withdrawal_with_batch_not_exists() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings.clone());

        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.run_redeem_stake_batch();
    }

    #[test]
    #[should_panic(expected = "illegal state - batch receipt does not exist")]
    fn run_redeem_batch_pending_withdrawal_with_batch_receipt_not_exists() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings.clone());

        *contract.batch_id_sequence += 1;
        contract.redeem_stake_batch = Some(RedeemStakeBatch::new(
            contract.batch_id_sequence,
            (10 * YOCTO).into(),
        ));
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        contract.run_redeem_stake_batch();
    }

    #[test]
    #[should_panic(expected = "unstaked funds are not yet available for withdrawal")]
    fn run_redeem_batch_pending_withdrawal_cannot_withdraw() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings.clone());

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
        contract.run_redeem_stake_batch();
    }

    #[test]
    fn refresh_stake_token_value() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = YOCTO;
        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(contract_settings.clone());
        contract.refresh_stake_token_value();

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
                    assert_eq!(method_name, "get_account_staked_balance");
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
                    assert_eq!(method_name, "on_refresh_account_staked_balance");
                    assert!(args.is_empty());
                }
                _ => panic!("expected func call action"),
            }
        }
    }
}
