use crate::core::Hash;
use crate::domain::{Account, StakeBatch};
use crate::interface::YoctoNear;
use crate::StakeTokenContract;
use crate::{
    domain,
    interface::{BatchId, RedeemStakeBatchReceipt, StakingService, YoctoStake},
};
use near_sdk::json_types::U128;
use near_sdk::{env, ext_contract, near_bindgen, AccountId, PromiseOrValue};

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
        self.insert_account(&account_hash, &account);
        batch_id
    }

    fn run_stake_batch(&mut self) -> PromiseOrValue<Option<BatchId>> {
        unimplemented!()
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
}

impl StakeTokenContract {
    fn batch_deposit_and_stake_request(&mut self, account_hash: Hash, account: Account) -> BatchId {
        // TODO
        unimplemented!()
    }

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

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn deposit_and_stake(&mut self);
}

#[ext_contract(ext_staking_pool_callbacks)]
pub trait ExtStakingPoolCallbacks {
    fn on_deposit_and_stake(&mut self, account_id: AccountId, stake_deposit: YoctoNear);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::Config;
    use crate::domain::StakeBatchReceipt;
    use crate::interface::AccountManagement;
    use crate::near::YOCTO;
    use crate::test_utils::{
        expected_account_storage_fee, near, Action, Receipt, EXPECTED_ACCOUNT_STORAGE_USAGE,
    };
    use near_sdk::json_types::ValidAccountId;
    use near_sdk::{serde_json, testing_env, AccountId, MockedBlockchain, VMContext};
    use std::convert::TryFrom;

    fn operator_id() -> AccountId {
        "operator.stake.oysterpack.near".to_string()
    }

    #[test]
    fn deposit_contract_not_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();
        let account = contract.lookup_account(valid_account_id.clone()).unwrap();
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(
            stake_batch.balance.balance.value(),
            context.attached_deposit
        );
        assert_eq!(stake_batch.id, batch_id);
    }

    #[test]
    fn deposit_contract_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
        contract.locked = true;

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();
        let account = contract.lookup_account(valid_account_id.clone()).unwrap();
        assert!(account.stake_batch.is_none());
        let stake_batch = account.next_stake_batch.unwrap();
        assert_eq!(
            stake_batch.balance.balance.value(),
            context.attached_deposit
        );
        assert_eq!(stake_batch.id, batch_id);
    }

    #[test]
    fn deposit_contract_not_locked_and_then_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let batch_id = contract.deposit();
        let account = contract.lookup_account(valid_account_id.clone()).unwrap();
        assert!(account.next_stake_batch.is_none());
        let stake_batch = account.stake_batch.unwrap();
        assert_eq!(
            stake_batch.balance.balance.value(),
            context.attached_deposit
        );
        assert_eq!(stake_batch.id, batch_id);

        contract.locked = true;

        context.attached_deposit = 50 * YOCTO;
        testing_env!(context.clone());

        let next_batch_id = contract.deposit();
        let account = contract.lookup_account(valid_account_id.clone()).unwrap();
        assert_eq!(account.stake_batch.unwrap().id, batch_id);
        let stake_batch = account.next_stake_batch.unwrap();
        assert_eq!(
            stake_batch.balance.balance.value(),
            context.attached_deposit
        );
        assert_eq!(stake_batch.id, next_batch_id);
    }

    /// Given the account has no funds in stake batches
    /// When funds are claimed
    /// Then there should be no effect
    #[test]
    fn claim_all_batch_receipt_funds_with_no_batched_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

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
        let mut context = near::new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.apply_stake_batch_credit(&mut account, YOCTO.into());
        contract.insert_account(&account_hash, &account);

        // When batch receipts are claimed
        contract.claim_all_batch_receipt_funds();
        // Then there should be no effect on the account
        let account = contract.lookup_account(valid_account_id.clone()).unwrap();
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
        let mut context = near::new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.apply_stake_batch_credit(&mut account, YOCTO.into());
        contract.insert_account(&account_hash, &account);

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
        let account = contract.lookup_account(valid_account_id.clone()).unwrap();
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
        contract.insert_account(&account_hash, &account);
        // When batch receipts are claimed
        contract.claim_all_batch_receipt_funds();
        // Assert
        let account = contract.lookup_account(valid_account_id.clone()).unwrap();
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
        let mut context = near::new_context(account_id);
        context.is_view = false;
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        // Given account has funds deposited into the current StakeBatch
        // And there are no receipts
        let account_hash = Hash::from(account_id);
        let mut account = contract.accounts.get(&account_hash).unwrap();
        let batch_id = contract.apply_stake_batch_credit(&mut account, (2 * YOCTO).into());
        contract.insert_account(&account_hash, &account);

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
        let account = contract.lookup_account(valid_account_id.clone()).unwrap();
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
    #[ignore]
    fn claim_all_batch_receipt_funds_with_stake_batch_and_next_stake_batch_funds_with_receipts() {
        unimplemented!()
    }
}
