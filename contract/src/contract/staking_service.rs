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

    fn deposit_and_stake(&mut self) -> PromiseOrValue<BatchId> {
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = self
            .accounts
            .get(&account_hash)
            .expect("account is not registered");

        assert!(
            env::attached_deposit() > 0,
            "deposit is required in order to stake"
        );

        self.claim_receipt_funds(&mut account);

        if self.locked {
            self.apply_stake_batch_credit(&mut account, env::attached_deposit().into());
            let batch_id = account.stake_batch.unwrap().id();
            self.insert_account(&account_hash, &account);
            PromiseOrValue::Value(batch_id.into())
        } else {
            self.locked = true;

            unimplemented!()
        }
    }

    fn run_stake_batch(&mut self) -> PromiseOrValue<bool> {
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

    fn run_redeem_stake_batch(&mut self) -> PromiseOrValue<bool> {
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
    fn apply_stake_batch_credit(&mut self, account: &mut Account, amount: domain::YoctoNear) {
        if amount.value() == 0 {
            return;
        }

        // apply to contract level batch
        {
            if self.locked {
                let mut batch = self.next_stake_batch.unwrap_or_else(|| {
                    // create the next batch
                    *self.batch_id_sequence += 1;
                    StakeBatch::new(self.batch_id_sequence, domain::YoctoNear(0))
                });
                batch.add(amount);
                self.next_stake_batch = Some(batch);
            } else {
                let mut batch = self.stake_batch.unwrap_or_else(|| {
                    // create the next batch
                    *self.batch_id_sequence += 1;
                    StakeBatch::new(self.batch_id_sequence, domain::YoctoNear(0))
                });
                batch.add(amount);
                self.stake_batch = Some(batch);
            }
        }

        let mut batch = account
            .stake_batch
            .unwrap_or_else(|| StakeBatch::new(self.batch_id_sequence, domain::YoctoNear(0)));
        batch.add(amount);
        account.stake_batch = Some(batch);
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
    fn claim_all_batch_receipt_funds() {
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
}
