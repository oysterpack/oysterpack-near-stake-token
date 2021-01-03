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
    fn account_storage_fee(&self) -> interface::YoctoNear {
        let fee = self.config.storage_cost_per_byte().value()
            * self.account_storage_usage.value() as u128;
        fee.into()
    }

    fn account_registered(&self, account_id: ValidAccountId) -> bool {
        self.accounts.get(&Hash::from(account_id)).is_some()
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
                    locked_stake: account.locked_stake.map(Into::into),
                    stake_batch: account.stake_batch.map(Into::into),
                    next_stake_batch: account.next_stake_batch.map(Into::into),
                    redeem_stake_batch,
                    next_redeem_stake_batch,
                    contract_near_liquidity,
                }
            })
    }

    fn withdraw(&mut self, amount: interface::YoctoNear) {
        let mut account = self.registered_account(&env::predecessor_account_id());
        self.withdraw_near_funds(&mut account, amount.into());
    }

    fn withdraw_all(&mut self) -> interface::YoctoNear {
        let mut account = self.registered_account(&env::predecessor_account_id());
        self.claim_receipt_funds(&mut account);
        match account.near {
            None => 0.into(),
            Some(balance) => {
                self.withdraw_near_funds(&mut account, balance.amount());
                balance.amount().into()
            }
        }
    }
}

impl StakeTokenContract {
    fn withdraw_near_funds(&mut self, account: &mut RegisteredAccount, amount: YoctoNear) {
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
mod test {
    use super::*;
    use crate::interface::StakingService;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{serde_json, testing_env, MockedBlockchain};
    use std::convert::TryInto;

    #[test]
    fn account_registered_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(None, contract_settings);

        context.is_view = true;
        testing_env!(context.clone());
        assert!(!contract.account_registered(account_id.try_into().unwrap()));
    }

    #[test]
    fn lookup_account_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(None, contract_settings);

        context.is_view = true;
        testing_env!(context.clone());
        assert!(contract
            .lookup_account(account_id.try_into().unwrap())
            .is_none());
    }

    #[test]
    fn lookup_account_with_unclaimed_receipts() {
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

        let batch = contract.stake_batch.unwrap();
        // create a stake batch receipt for the stake batch
        let receipt =
            domain::StakeBatchReceipt::new(batch.balance().amount(), contract.stake_token_value);
        contract.stake_batch_receipts.insert(&batch.id(), &receipt);
        contract.stake_batch = None;

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
        let mut account = contract.registered_account(account_id);
        account.redeem_stake_batch = Some(redeem_stake_batch);
        contract.save_account(&account.id, &account);

        context.is_view = true;
        testing_env!(context.clone());
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        assert!(account.redeem_stake_batch.is_none());
        assert_eq!(account.stake.unwrap().amount, (10 * YOCTO).into());
        assert_eq!(account.near.unwrap().amount, (2 * YOCTO).into());
    }

    #[test]
    fn lookup_account_with_unclaimed_receipts_pending_withdrawal() {
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

        let batch = contract.stake_batch.unwrap();
        // create a stake batch receipt for the stake batch
        let receipt =
            domain::StakeBatchReceipt::new(batch.balance().amount(), contract.stake_token_value);
        contract.stake_batch_receipts.insert(&batch.id(), &receipt);
        contract.stake_batch = None;

        // create a redeem stake batch receipt for 2 yoctoSTAKE
        *contract.batch_id_sequence += 1;
        let redeem_stake_batch =
            domain::RedeemStakeBatch::new(contract.batch_id_sequence, (2 * YOCTO).into());
        contract.redeem_stake_batch = Some(redeem_stake_batch);
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &domain::RedeemStakeBatchReceipt::new(
                redeem_stake_batch.balance().amount(),
                contract.stake_token_value,
            ),
        );
        contract.run_redeem_stake_batch_lock = Some(RedeemLock::PendingWithdrawal);
        let mut account = contract.registered_account(account_id);
        account.redeem_stake_batch = Some(redeem_stake_batch);
        contract.save_account(&account.id, &account);

        context.is_view = true;
        testing_env!(context.clone());
        let account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert!(account.stake_batch.is_none());
        assert_eq!(account.stake.unwrap().amount, (10 * YOCTO).into());
        assert!(account.near.is_none());
        account
            .redeem_stake_batch
            .unwrap()
            .receipt
            .expect("receipt should be present");
    }

    #[test]
    fn account_storage_fee_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(None, contract_settings);

        context.is_view = true;
        testing_env!(context.clone());
        contract.account_storage_fee();
    }

    #[test]
    fn total_registered_accounts_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(None, contract_settings);

        context.is_view = true;
        testing_env!(context.clone());
        contract.total_registered_accounts();
    }

    /// - Given the account is not currently registered
    /// - When a new account is registered with attached deposit to stake
    /// - Then [AccountRegistry::account_registered()] returns true for the registered account ID
    /// - And the total accounts registered count is incremented
    /// - And the storage fee credit is applied on the account and on the contract
    /// - And the account deposit minus the storage fee is credited to the stake batch on the account
    ///   and the on the contract
    /// - And the next stake batch is set to None
    /// - And the redeem stake batches are set to None
    #[test]
    fn register_new_account() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        // Given the account is not currently registered
        assert!(
            !contract.account_registered(account_id.try_into().unwrap()),
            "account should not be registered"
        );

        let storage_before_registering_account = env::storage_usage();
        contract.register_account();

        // the txn should have created a Transfer receipt to refund the storage fee over payment
        let receipt = env::created_receipts().first().cloned().unwrap();
        let json = serde_json::to_string_pretty(&receipt).unwrap();
        println!("receipt: {}", json);
        let receipt: Receipt = serde_json::from_str(&json).unwrap();
        let refund: u128 = match receipt.actions.first().unwrap() {
            Action::Transfer { deposit } => *deposit,
            action => panic!("unexpected action: {:?}", action),
        };
        assert_eq!(
            refund,
            context.attached_deposit - contract.account_storage_fee().value()
        );

        let account = contract
            .accounts
            .get(&Hash::from(account_id))
            .expect("account should be registered");
        assert!(
            contract.account_registered(account_id.try_into().unwrap()),
            "account should be registered"
        );
        assert_eq!(
            contract.total_registered_accounts().0,
            1,
            "There should be 1 account registered"
        );

        let account_storage_usage = env::storage_usage() - storage_before_registering_account;
        assert_eq!(
            account_storage_usage, 120,
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
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();
        println!("{:?}", env::created_receipts());
        assert!(env::created_receipts().is_empty());
    }

    #[test]
    #[should_panic(expected = "account is already registered")]
    fn register_preexisting_account() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();
        contract.register_account();
    }

    #[test]
    #[should_panic(expected = "sufficient deposit is required to pay for account storage fees")]
    fn register_account_with_no_attached_deposit() {
        let account_id = "alfio-zappala.near";
        let context = new_context(account_id);
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();
    }

    #[test]
    #[should_panic(expected = "sufficient deposit is required to pay for account storage fees")]
    fn register_account_with_insufficient_deposit_for_storage_fees() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 1;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();
    }

    #[test]
    fn lookup_account() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        assert!(contract
            .lookup_account(account_id.try_into().unwrap())
            .is_none());
        contract.register_account();

        let stake_account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        let stake_account_json = serde_json::to_string_pretty(&stake_account).unwrap();
        println!("{}", stake_account_json);
    }

    #[test]
    fn unregister_registered_account_with_no_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        assert!(contract
            .lookup_account(account_id.try_into().unwrap())
            .is_none());
        contract.register_account();
        assert!(contract.account_registered(account_id.try_into().unwrap()));
        let stake_account = contract
            .lookup_account(account_id.try_into().unwrap())
            .unwrap();
        assert!(stake_account.stake_batch.is_none());
        assert!(stake_account.near.is_none());
        assert!(stake_account.stake.is_none());

        contract.unregister_account();
        assert!(!contract.account_registered(account_id.try_into().unwrap()));
        assert_eq!(
            env::account_balance(),
            context.account_balance,
            "storage fees should have been refunded"
        );
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn unregister_account_with_staked_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = expected_account_storage_fee() + 1;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        // given the account has STAKE funds
        let contract_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&contract_hash).unwrap();
        account.apply_stake_credit(1.into());
        contract.save_account(&contract_hash, &account);

        // then unregister will fail
        contract.unregister_account();
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn unregister_account_with_stake_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // credit some STAKE
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&account_hash).unwrap();
        account.apply_stake_credit(1.into());
        contract.accounts.insert(&account_hash, &account);
        contract.unregister_account();
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn unregister_account_with_near_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // credit some NEAR
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&account_hash).unwrap();
        account.apply_near_credit(1.into());
        contract.accounts.insert(&account_hash, &account);
        contract.unregister_account();
    }

    #[test]
    #[should_panic(expected = "account is not registered")]
    fn unregister_unknown_account() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = expected_account_storage_fee() + 1;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.unregister_account();
    }

    #[test]
    fn withdraw_partial_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // Given the account has some NEAR balance
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&account_hash).unwrap();
        account.apply_near_credit((10 * YOCTO).into());
        contract.accounts.insert(&account_hash, &account);
        contract.total_near.credit(account.near.unwrap().amount());

        // When partial funds are withdrawn
        contract.withdraw((5 * YOCTO).into());
        // Assert that the account NEAR balance was debited
        let account = contract.accounts.get(&account_hash).unwrap();
        assert_eq!(account.near.unwrap().amount(), (5 * YOCTO).into());
    }

    #[test]
    fn withdraw_all_has_near_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // Given the account has some NEAR balance
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&account_hash).unwrap();
        account.apply_near_credit((10 * YOCTO).into());
        contract.accounts.insert(&account_hash, &account);
        contract.total_near.credit(account.near.unwrap().amount());

        contract.withdraw_all();
        // Assert that the account NEAR balance was debited
        let account = contract.accounts.get(&account_hash).unwrap();
        assert!(account.near.is_none());
    }

    #[test]
    fn withdraw_all_has_near_funds_in_unclaimed_receipts() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // Given the account has some NEAR balance
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&account_hash).unwrap();
        *contract.batch_id_sequence += 1;
        account.redeem_stake_batch = Some(RedeemStakeBatch::new(
            contract.batch_id_sequence,
            YOCTO.into(),
        ));
        contract.accounts.insert(&account_hash, &account);
        contract.total_near.credit(YOCTO.into());
        contract.redeem_stake_batch_receipts.insert(
            &contract.batch_id_sequence,
            &RedeemStakeBatchReceipt::new(YOCTO.into(), contract.stake_token_value),
        );

        contract.withdraw_all();
        // Assert that the account NEAR balance was debited
        let account = contract.accounts.get(&account_hash).unwrap();
        assert!(account.near.is_none());
    }

    #[test]
    #[should_panic(expected = "account NEAR balance is too low to fulfill request")]
    fn withdraw_with_no_near_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();
        contract.withdraw((50 * YOCTO).into());
    }

    #[test]
    #[should_panic(expected = "account NEAR balance is too low to fulfill request")]
    fn withdraw_with_insufficient_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();

        // Given the account has some NEAR balance
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&account_hash).unwrap();
        account.apply_near_credit((10 * YOCTO).into());
        contract.accounts.insert(&account_hash, &account);

        contract.withdraw((50 * YOCTO).into());
    }

    #[test]
    fn withdraw_all_with_no_near_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);

        contract.register_account();
        assert_eq!(contract.withdraw_all().value(), 0);
    }
}
