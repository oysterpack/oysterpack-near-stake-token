//required in order for near_bindgen macro to work outside of lib.rs
use crate::domain::RegisteredAccount;
use crate::errors::account_management::ACCOUNT_NOT_REGISTERED;
use crate::*;
use crate::{
    core::Hash,
    domain::{Account, YoctoNear},
    errors::account_management::{
        ACCOUNT_ALREADY_REGISTERED, INSUFFICIENT_STORAGE_FEE, UNREGISTER_REQUIRES_ZERO_BALANCES,
    },
    interface::{self, AccountManagement, StakeAccount, StakingService},
};
use near_sdk::{
    env,
    json_types::{ValidAccountId, U128},
    near_bindgen, Promise,
};

#[near_bindgen]
impl AccountManagement for StakeTokenContract {
    /// ## Logic
    /// - check attached deposit
    ///   - assert amount is enough to cover storage fees
    /// - track the account storage fees
    /// - refunds funds minus account storage fees
    ///
    /// ## Panics
    /// - if attached deposit is not enough to cover account storage fees
    /// - if account is already registered
    #[payable]
    fn register_account(&mut self) {
        assert!(
            env::attached_deposit() >= self.account_storage_fee().value(),
            INSUFFICIENT_STORAGE_FEE,
        );

        let account_storage_fee = self.account_storage_fee().into();
        let account = Account::new(account_storage_fee);
        assert!(
            self.save_account(&Hash::from(&env::predecessor_account_id()), &account),
            ACCOUNT_ALREADY_REGISTERED
        );

        // refund over payment of storage fees
        let refund = env::attached_deposit() - account_storage_fee.value();
        if refund > 0 {
            Promise::new(env::predecessor_account_id()).transfer(refund);
        }
    }

    fn unregister_account(&mut self) {
        let account_id = env::predecessor_account_id();
        let account_id_hash = Hash::from(&env::predecessor_account_id());

        match self.delete_account(&account_id_hash) {
            None => panic!(ACCOUNT_NOT_REGISTERED),
            Some(account) => {
                assert!(!account.has_funds(), UNREGISTER_REQUIRES_ZERO_BALANCES);
                // refund the escrowed storage fee
                Promise::new(account_id).transfer(account.storage_escrow.amount().value());
            }
        };
    }

    /// returns the required account storage fee that needs to be attached to the account registration
    /// contract function call in yoctoNEAR
    ///
    /// NOTE: this is dynamic based on the storage cost per byte specified in the config
    fn account_storage_fee(&self) -> interface::YoctoNear {
        let fee = self.config.storage_cost_per_byte().value()
            * self.account_storage_usage.value() as u128;
        fee.into()
    }

    fn account_registered(&self, account_id: ValidAccountId) -> bool {
        self.accounts.contains_key(&Hash::from(account_id))
    }

    fn total_registered_accounts(&self) -> U128 {
        self.accounts_len.into()
    }

    fn lookup_account(&self, account_id: ValidAccountId) -> Option<StakeAccount> {
        self.accounts
            .get(&Hash::from(account_id))
            .map(|account| self.apply_receipt_funds_for_view(&account))
            .map(|account| {
                let redeem_stake_batch = account.redeem_stake_batch.map(|batch| {
                    interface::RedeemStakeBatch::from(
                        batch,
                        self.redeem_stake_batch_receipt(batch.id().into()),
                    )
                });

                let next_redeem_stake_batch = account.next_redeem_stake_batch.map(|batch| {
                    interface::RedeemStakeBatch::from(
                        batch,
                        self.redeem_stake_batch_receipt(batch.id().into()),
                    )
                });

                let contract_near_liquidity = if self.near_liquidity_pool.value() == 0 {
                    None
                } else {
                    let mut total_unstaked_near = YoctoNear(0);

                    let mut update_total_unstaked_near = |batch: &interface::RedeemStakeBatch| {
                        if let Some(receipt) = batch.receipt.as_ref() {
                            let stake_token_value: domain::StakeTokenValue =
                                receipt.stake_token_value.clone().into();
                            total_unstaked_near +=
                                stake_token_value.stake_to_near(receipt.redeemed_stake.0 .0.into());
                        }
                    };

                    if let Some(batch) = redeem_stake_batch.as_ref() {
                        update_total_unstaked_near(batch);
                    }

                    if let Some(batch) = next_redeem_stake_batch.as_ref() {
                        update_total_unstaked_near(batch);
                    }

                    if total_unstaked_near.value() > 0 {
                        if self.near_liquidity_pool.value() >= total_unstaked_near.value() {
                            Some(total_unstaked_near.into())
                        } else {
                            Some(self.near_liquidity_pool.into())
                        }
                    } else {
                        None
                    }
                };

                StakeAccount {
                    storage_escrow: account.storage_escrow.into(),
                    near: account.near.map(Into::into),
                    stake: account.stake.map(Into::into),
                    stake_batch: account.stake_batch.map(Into::into),
                    next_stake_batch: account.next_stake_batch.map(Into::into),
                    redeem_stake_batch,
                    next_redeem_stake_batch,
                    contract_near_liquidity,
                }
            })
    }
}

impl StakeTokenContract {
    /// ## Panics
    /// if account is not registered
    pub(crate) fn registered_account(&self, account_id: &str) -> RegisteredAccount {
        let account_id_hash = Hash::from(account_id);
        match self.accounts.get(&Hash::from(account_id)) {
            Some(account) => RegisteredAccount {
                account,
                id: account_id_hash,
            },
            None => panic!("{}: {}", ACCOUNT_NOT_REGISTERED, account_id),
        }
    }

    /// returns true if this was a new account
    fn save_account(&mut self, account_id: &Hash, account: &Account) -> bool {
        if self.accounts.insert(account_id, account).is_none() {
            // new account was added
            self.accounts_len += 1;
            return true;
        }
        false
    }

    pub(crate) fn save_registered_account(&mut self, account: &RegisteredAccount) {
        self.save_account(&account.id, &account.account);
    }

    /// returns the account that was deleted, or None if no account exists for specified account ID
    fn delete_account(&mut self, account_id: &Hash) -> Option<Account> {
        self.accounts.remove(account_id).map(|account| {
            self.accounts_len -= 1;
            account
        })
    }
}

#[cfg(test)]
mod test_register_account {
    use super::*;
    use crate::interface::AccountManagement;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::test_utils::get_created_receipts;
    use near_sdk::{testing_env, MockedBlockchain};
    use std::convert::TryInto;

    /// When a user registers a new account
    /// And attaches more then the required payment for account storage
    /// Then the difference will be refunded
    #[test]
    fn register_new_account_with_deposit_overpayment() {
        let mut test_context = TestContext::new(None);
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;
        let account_id = test_context.account_id;

        // Given the account is not currently registered
        assert!(
            !contract.account_registered(account_id.try_into().unwrap()),
            "account should not be registered"
        );

        // measure how much actual storage is consumed by the new account
        let storage_before_registering_account = env::storage_usage();
        // desposit is required for registering the account - 1 NEAR is more than enough
        // the account will be refunded the difference
        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.register_account();

        // the txn should have created a Transfer receipt to refund the storage fee over payment
        let receipts = deserialize_receipts(&get_created_receipts());
        assert_eq!(receipts.len(), 1);
        let receipt = &receipts[0];
        match receipt.actions.first().unwrap() {
            Action::Transfer { deposit } => assert_eq!(
                *deposit,
                context.attached_deposit - contract.account_storage_fee().value()
            ),
            action => panic!("unexpected action: {:?}", action),
        };

        let account = contract.registered_account(account_id);
        assert_eq!(
            contract.total_registered_accounts().0,
            1,
            "There should be 1 account registered"
        );

        let account_storage_usage = env::storage_usage() - storage_before_registering_account;
        assert_eq!(
            account_storage_usage, 119,
            "account storage usage changed !!! If the change is expected, then update the assert"
        );

        // And the storage fee credit is applied on the account
        assert_eq!(
            account.storage_escrow.amount(),
            contract.account_storage_fee().into()
        );
    }

    #[test]
    fn register_account_with_exact_storage_fee() {
        let mut test_context = TestContext::new(None);
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.attached_deposit = contract.account_storage_fee().value();
        testing_env!(context.clone());
        contract.register_account();

        // no refund is expected
        assert!(get_created_receipts().is_empty());
    }

    #[test]
    #[should_panic(expected = "account is already registered")]
    fn register_preexisting_account() {
        let mut test_context = TestContext::with_registered_account(None);
        let mut context = test_context.context.clone();

        context.attached_deposit = YOCTO;
        testing_env!(context);
        test_context.contract.register_account();
    }

    #[test]
    #[should_panic(expected = "sufficient deposit is required to pay for account storage fees")]
    fn register_account_with_no_attached_deposit() {
        let mut test_context = TestContext::new(None);
        test_context.contract.register_account();
    }

    #[test]
    #[should_panic(expected = "sufficient deposit is required to pay for account storage fees")]
    fn register_account_with_insufficient_deposit_for_storage_fees() {
        let mut test_context = TestContext::new(None);
        test_context.context.attached_deposit = 1;
        testing_env!(test_context.context.clone());
        test_context.contract.register_account();
    }
}

#[cfg(test)]
mod test_unregister_account {
    use super::*;
    use crate::interface::AccountManagement;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::test_utils::get_created_receipts;
    use near_sdk::{testing_env, MockedBlockchain};
    use std::convert::TryInto;
    use std::ops::DerefMut;

    #[test]
    fn unregister_registered_account_with_no_funds() {
        let test_context = TestContext::with_registered_account(None);
        let mut contract = test_context.contract;

        contract.unregister_account();
        assert!(!contract.account_registered(test_context.account_id.try_into().unwrap()));
        let receipts = deserialize_receipts(&get_created_receipts());
        // account storage fee should have been refunded
        assert_eq!(receipts.len(), 1);
        let receipt = &receipts[0];
        assert_eq!(&receipt.receiver_id, test_context.account_id);
        match &receipt.actions[0] {
            Action::Transfer { deposit } => {
                assert_eq!(*deposit, contract.account_storage_fee().value())
            }
            _ => panic!("expected account storage fee to be refunded"),
        }
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn unregister_account_with_stake_funds() {
        let mut test_context = TestContext::with_registered_account(None);
        let contract = &mut test_context.contract;

        // apply STAKE credit to the account
        let mut registered_account = contract.registered_account(test_context.account_id);
        registered_account.account.apply_stake_credit(1.into());
        contract.save_registered_account(&registered_account);

        // then unregister will fail
        contract.unregister_account();
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn unregister_account_with_near_funds() {
        let mut test_context = TestContext::with_registered_account(None);
        let contract = &mut test_context.contract;

        // credit some NEAR
        let mut account = contract.registered_account(test_context.account_id);
        account.deref_mut().apply_near_credit(1.into());
        contract.save_registered_account(&account);

        // unregister should fail
        contract.unregister_account();
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn account_has_funds_in_stake_batch() {
        let mut test_context = TestContext::with_registered_account(None);
        let mut context = test_context.context;
        let contract = &mut test_context.contract;

        // credit some NEAR
        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        contract.deposit();

        // unregister should fail
        contract.unregister_account();
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn account_has_funds_in_next_stake_batch() {
        let mut test_context = TestContext::with_registered_account(None);
        let mut context = test_context.context;
        let contract = &mut test_context.contract;

        // credit some NEAR
        context.attached_deposit = YOCTO;
        testing_env!(context.clone());
        // setting the lock to true should cause the deposit to be put in the next stake batch
        contract.run_stake_batch_locked = true;
        contract.deposit();
        // confirm that account has funds in next stake batch
        let registered_account = contract.registered_account(test_context.account_id);
        assert!(registered_account.account.next_stake_batch.is_some());

        // unregister should fail
        contract.unregister_account();
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn account_has_funds_in_redeem_stake_batch() {
        let mut test_context = TestContext::with_registered_account(None);
        let contract = &mut test_context.contract;

        // give the account STAKE
        let mut registered_account = contract.registered_account(test_context.account_id);
        registered_account.apply_stake_credit(YOCTO.into());
        contract.save_registered_account(&registered_account);
        // then redeem it to move the STAKE funds in the redeem stake batch
        contract.redeem_all();

        // unregister should fail
        contract.unregister_account();
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn account_has_funds_in_next_redeem_stake_batch() {
        let mut test_context = TestContext::with_registered_account(None);
        let contract = &mut test_context.contract;

        // give the account STAKE
        let mut registered_account = contract.registered_account(test_context.account_id);
        registered_account.apply_stake_credit(YOCTO.into());
        contract.save_registered_account(&registered_account);
        // set lock to pending withdrawal to force STAKE funds to go into the next redeem batch
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        // pending withdrawal requires redeem stake batch to be present
        contract.redeem_stake_batch = Some(RedeemStakeBatch::new(
            contract.batch_id_sequence,
            YOCTO.into(),
        ));
        // then redeem it to move the STAKE funds in the redeem stake batch
        contract.redeem_all();
        // confirm there is a next redeem stake batch
        let registered_account = contract.registered_account(test_context.account_id);
        assert!(registered_account.account.next_redeem_stake_batch.is_some());

        // unregister should fail
        contract.unregister_account();
    }

    #[test]
    #[should_panic(expected = "account is not registered")]
    fn unregister_unknown_account() {
        let mut test_context = TestContext::new(None);
        test_context.contract.unregister_account();
    }
}

#[cfg(test)]
mod test_lookup_account {
    use super::*;
    use crate::interface::{AccountManagement, StakingService};
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};
    use std::convert::TryInto;

    #[test]
    fn lookup_registered_account() {
        let test_context = TestContext::with_registered_account(None);

        test_context
            .contract
            .lookup_account(test_context.account_id.try_into().unwrap())
            .expect("account should be registered");
    }

    #[test]
    fn lookup_unregistered_account() {
        let test_context = TestContext::new(None);

        assert!(test_context
            .contract
            .lookup_account(test_context.account_id.try_into().unwrap())
            .is_none());
    }

    /// when an account has unclaimed receipts, the receipts are applied to the account balances
    /// for display purposes - so as not to confuse the end user
    #[test]
    fn with_unclaimed_receipts() {
        let mut ctx = TestContext::with_registered_account(None);
        let mut context = ctx.context;
        let contract = &mut ctx.contract;

        // setup receipts for the account
        {
            // deposit funds into a stake batch
            context.attached_deposit = 10_u128 * YOCTO;
            testing_env!(context.clone());
            contract.deposit();

            // simulate that the batch was processed and create a batch receipt for it
            let batch = contract.stake_batch.unwrap();
            // create a stake batch receipt for the stake batch
            let receipt = domain::StakeBatchReceipt::new(
                batch.balance().amount(),
                contract.stake_token_value,
            );
            contract.stake_batch_receipts.insert(&batch.id(), &receipt);
            contract.stake_batch = None;

            // credit the account with some STAKE and then redeem it
            let mut registered_account = contract.registered_account(ctx.account_id);
            registered_account
                .account
                .apply_stake_credit((YOCTO * 2).into());
            contract.save_registered_account(&registered_account);
            contract.redeem((YOCTO * 2).into());

            // create a receipt for the batch
            let redeem_stake_batch_receipt = contract
                .redeem_stake_batch
                .unwrap()
                .create_receipt(contract.stake_token_value);
            contract
                .redeem_stake_batch_receipts
                .insert(&contract.batch_id_sequence, &redeem_stake_batch_receipt);
        }

        context.is_view = true;
        testing_env!(context.clone());
        let account = contract
            .lookup_account(ctx.account_id.try_into().unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        assert!(account.redeem_stake_batch.is_none());
        assert_eq!(account.stake.unwrap().amount, (10_u128 * YOCTO).into());
        assert_eq!(account.near.unwrap().amount, (2_u128 * YOCTO).into());
    }

    #[test]
    fn with_unclaimed_receipts_pending_withdrawal() {
        let mut ctx = TestContext::with_registered_account(None);
        let mut context = ctx.context;
        let contract = &mut ctx.contract;

        // setup
        {
            // credit the account some STAKE and then redeem it all
            let mut registered_account = contract.registered_account(ctx.account_id);
            registered_account
                .account
                .apply_stake_credit((YOCTO * 10).into());
            contract.save_registered_account(&registered_account);

            contract.redeem_all();

            let batch = contract.redeem_stake_batch.unwrap();
            let receipt = batch.create_receipt(contract.stake_token_value);
            contract
                .redeem_stake_batch_receipts
                .insert(&batch.id(), &receipt);
            contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        }

        context.is_view = true;
        testing_env!(context.clone());
        let account = contract
            .lookup_account(ctx.account_id.try_into().unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        assert!(account.stake.is_none());
        assert!(account.near.is_none());
        account
            .redeem_stake_batch
            .unwrap()
            .receipt
            .expect("receipt for pending withdrawal should be present");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interface::AccountManagement;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};
    use std::convert::TryFrom;

    /// the following contract funcs are expected to be invoked in view mode:
    /// - account_storage_fee
    /// - account_registered
    /// - total_registered_accounts
    /// - lookup_account
    #[test]
    fn check_view_funcs() {
        let mut ctx = TestContext::new(None);

        // given the funcs are called in view mode
        ctx.context.is_view = true;
        testing_env!(ctx.context.clone());
        ctx.contract.account_storage_fee();
        ctx.contract.total_registered_accounts();
        ctx.contract
            .lookup_account(ValidAccountId::try_from(ctx.account_id).unwrap());
    }
}
