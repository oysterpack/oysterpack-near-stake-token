use crate::domain::{
    BatchId, RedeemStakeBatch, RedeemStakeBatchReceipt, StakeBatch, StakeBatchReceipt,
};
use crate::interface::StakeAccount;
use crate::near::YOCTO;
use crate::{
    core::Hash,
    domain::{Account, StorageUsage, YoctoNear, YoctoNearValue},
    interface::{self, AccountManagement},
    StakeTokenContract,
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
        let account_storage_fee: YoctoNear = self.account_storage_fee().into();
        let attached_deposit = YoctoNear(env::attached_deposit());
        assert!(
            attached_deposit.value() >= account_storage_fee.value(),
            "deposit is required to pay for account storage fees : {} NEAR",
            account_storage_fee.value() as f64 / YOCTO as f64,
        );

        let account = Account::new(account_storage_fee);
        assert!(
            self.insert_account(&Hash::from(&env::predecessor_account_id()), &account)
                .is_none(),
            "account is already registered"
        );

        // refund over payment of storage fees
        let refund = attached_deposit - account_storage_fee;
        if refund.value() > 0 {
            Promise::new(env::predecessor_account_id()).transfer(refund.value());
        }
    }

    fn unregister_account(&mut self) {
        let account_id = env::predecessor_account_id();
        let account_id_hash = Hash::from(&env::predecessor_account_id());

        match self.remove_account(&account_id_hash) {
            None => panic!("account is not registered"),
            Some(account) => {
                assert!(
                    !account.has_funds(),
                    "all funds must be withdrawn from the account in order to unregister"
                );
                // refund the escrowed storage fee
                Promise::new(account_id).transfer(account.storage_escrow.balance().value());
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
        let hash = Hash::from(account_id.as_ref());
        self.accounts.get(&hash).is_some()
    }

    fn total_registered_accounts(&self) -> U128 {
        self.accounts_len.into()
    }

    fn lookup_account(&self, account_id: ValidAccountId) -> Option<StakeAccount> {
        let hash = Hash::from(account_id.as_ref());
        self.accounts.get(&hash).map(Into::into)
    }

    fn withdraw(&mut self, amount: interface::YoctoNear) -> Promise {
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = self.registered_account(&account_hash);
        match account.near {
            None => panic!("there are no available NEAR funds to withdraw"),
            Some(_) => self.withdraw_near_funds(&mut account, &account_hash, amount.into()),
        }
    }

    fn withdraw_all(&mut self) -> Promise {
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = self.registered_account(&account_hash);
        match account.near {
            None => panic!("there are no available NEAR funds to withdraw"),
            Some(balance) => {
                self.withdraw_near_funds(&mut account, &account_hash, balance.balance())
            }
        }
    }
}

impl StakeTokenContract {
    fn withdraw_near_funds(
        &mut self,
        account: &mut Account,
        account_hash: &Hash,
        amount: YoctoNear,
    ) -> Promise {
        account.apply_near_debit(amount);
        self.insert_account(&account_hash, &account);
        self.total_near.debit(amount);
        Promise::new(env::predecessor_account_id()).transfer(amount.value())
    }

    fn registered_account(&self, account_hash: &Hash) -> Account {
        self.accounts
            .get(&account_hash)
            .expect("account is not registered")
    }

    /// when a new account is registered the following is tracked:
    /// - total account count is inc
    /// - total storage escrow is updated
    pub(crate) fn insert_account(
        &mut self,
        account_id: &Hash,
        account: &Account,
    ) -> Option<Account> {
        match self.accounts.insert(account_id, account) {
            None => {
                self.accounts_len += 1;
                self.total_storage_escrow
                    .credit(account.storage_escrow.balance());
                None
            }
            Some(previous) => Some(previous),
        }
    }

    /// when a new account is registered the following is tracked:
    /// - total account count is dev
    /// - total storage escrow is updated
    fn remove_account(&mut self, account_id: &Hash) -> Option<Account> {
        match self.accounts.remove(account_id) {
            None => None,
            Some(account) => {
                self.accounts_len -= 1;
                self.total_storage_escrow
                    .debit(account.storage_escrow.balance());
                Some(account)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::Config;
    use crate::near::YOCTO;
    use crate::test_utils::{
        expected_account_storage_fee, near, Action, Receipt, EXPECTED_ACCOUNT_STORAGE_USAGE,
    };
    use near_sdk::{serde_json, testing_env, AccountId, MockedBlockchain, VMContext};
    use std::convert::TryFrom;

    fn operator_id() -> AccountId {
        "operator.stake.oysterpack.near".to_string()
    }

    #[test]
    fn account_registered_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        context.is_view = true;
        testing_env!(context.clone());
        assert!(!contract.account_registered(valid_account_id.clone()));
    }

    #[test]
    fn lookup_account_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        context.is_view = true;
        testing_env!(context.clone());
        assert!(contract.lookup_account(valid_account_id.clone()).is_none());
    }

    #[test]
    fn account_storage_fee_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        context.is_view = true;
        testing_env!(context.clone());
        contract.account_storage_fee();
    }

    #[test]
    fn total_registered_accounts_is_view_func() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        context.is_view = true;
        testing_env!(context.clone());
        contract.total_registered_accounts();
    }

    /// - Given the contract is not locked
    /// - And the account is not currently registered
    /// - When a new account is registered with attached deposit to stake
    /// - Then [AccountRegistry::account_registered()] returns true for the registered account ID
    /// - And the total accounts registered count is incremented
    /// - And the storage fee credit is applied on the account and on the contract
    /// - And the account deposit minus the storage fee is credited to the stake batch on the account
    ///   and the on the contract
    /// - And the next stake batch is set to None
    /// - And the redeem stake batches are set to None
    #[test]
    fn register_account_when_contract_not_locked() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        // Given the contract is not locked
        assert!(!contract.locked);
        // And the account is not currently registered
        assert!(
            !contract.account_registered(valid_account_id.clone()),
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
            contract.account_registered(valid_account_id.clone()),
            "account should be registered"
        );
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

        // And the storage fee credit is applied on the account and on the contract
        assert_eq!(
            account.storage_escrow.balance(),
            contract.account_storage_fee().into()
        );
        assert_eq!(
            contract.total_storage_escrow.balance(),
            contract.account_storage_fee().into()
        );
    }

    #[test]
    fn register_account_with_exact_storage_fee() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
        println!("{:?}", env::created_receipts());
        assert!(env::created_receipts().is_empty());
    }

    #[test]
    #[should_panic(expected = "account is already registered")]
    fn register_preexisting_account() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
        contract.register_account();
    }

    #[test]
    #[should_panic(expected = "deposit is required to pay for account storage fees")]
    fn register_account_with_no_attached_deposit() {
        let account_id = "alfio-zappala.near";
        let context = near::new_context(account_id);
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
    }

    #[test]
    #[should_panic(expected = "deposit is required to pay for account storage fees")]
    fn register_account_with_insufficient_deposit_for_storage_fees() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 1;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
    }

    #[test]
    fn lookup_account() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 10 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        assert!(contract.lookup_account(valid_account_id.clone()).is_none());
        contract.register_account();

        let stake_account = contract.lookup_account(valid_account_id.clone()).unwrap();
        let stake_account_json = serde_json::to_string_pretty(&stake_account).unwrap();
        println!("{}", stake_account_json);
    }

    #[test]
    fn unregister_registered_account_with_no_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        assert!(contract.lookup_account(valid_account_id.clone()).is_none());
        contract.register_account();
        assert!(contract.account_registered(valid_account_id.clone()));
        let stake_account = contract.lookup_account(valid_account_id.clone()).unwrap();
        assert!(stake_account.stake_batch.is_none());
        assert!(stake_account.near.is_none());
        assert!(stake_account.stake.is_none());

        let contract_balance_with_registered_account = env::account_balance();
        assert_eq!(
            contract.total_storage_escrow.balance().value(),
            contract_balance_with_registered_account
        );
        contract.unregister_account();
        assert!(!contract.account_registered(valid_account_id.clone()));
        assert_eq!(
            contract.total_storage_escrow.balance().value(),
            0,
            "storage fees should have been refunded"
        );
        assert_eq!(
            env::account_balance(),
            0,
            "storage fees should have been refunded"
        );
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn unregister_account_with_staked_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = expected_account_storage_fee() + 1;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        // given the account has STAKE funds
        let contract_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&contract_hash).unwrap();
        account.apply_stake_credit(1.into());
        contract.insert_account(&contract_hash, &account);

        // then unregister will fail
        contract.unregister_account();
    }

    #[test]
    #[should_panic(
        expected = "all funds must be withdrawn from the account in order to unregister"
    )]
    fn unregister_account_with_stake_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

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
        let mut context = near::new_context(account_id);
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

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
        let mut context = near::new_context(account_id);
        context.attached_deposit = expected_account_storage_fee() + 1;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.unregister_account();
    }

    #[test]
    fn withdraw_partial_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        // Given the account has some NEAR balance
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&account_hash).unwrap();
        account.apply_near_credit((10 * YOCTO).into());
        contract.accounts.insert(&account_hash, &account);
        contract.total_near.credit(account.near.unwrap().balance());

        // When partial funds are withdrawn
        contract.withdraw((5 * YOCTO).into());
        // Assert that the account NEAR balance was debited
        let account = contract.accounts.get(&account_hash).unwrap();
        assert_eq!(account.near.unwrap().balance(), (5 * YOCTO).into());
    }

    #[test]
    fn withdraw_all_has_near_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.attached_deposit = expected_account_storage_fee();
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        // Given the account has some NEAR balance
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&account_hash).unwrap();
        account.apply_near_credit((10 * YOCTO).into());
        contract.accounts.insert(&account_hash, &account);
        contract.total_near.credit(account.near.unwrap().balance());

        contract.withdraw_all();
        // Assert that the account NEAR balance was debited
        let account = contract.accounts.get(&account_hash).unwrap();
        assert!(account.near.is_none());
    }

    #[test]
    #[should_panic(expected = "there are no available NEAR funds to withdraw")]
    fn withdraw_with_no_near_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
        contract.withdraw((50 * YOCTO).into());
    }

    #[test]
    #[should_panic(expected = "balance is too low to fulfill debit request")]
    fn withdraw_with_insufficient_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();

        // Given the account has some NEAR balance
        let account_hash = Hash::from(&env::predecessor_account_id());
        let mut account = contract.accounts.get(&account_hash).unwrap();
        account.apply_near_credit((10 * YOCTO).into());
        contract.accounts.insert(&account_hash, &account);

        contract.withdraw((50 * YOCTO).into());
    }

    #[test]
    #[should_panic(expected = "there are no available NEAR funds to withdraw")]
    fn withdraw_all_with_no_near_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = near::new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let staking_pool_id = ValidAccountId::try_from("staking-pool.near").unwrap();
        let operator_id = ValidAccountId::try_from("nob.near").unwrap();
        let valid_account_id = ValidAccountId::try_from(account_id).unwrap();
        let mut contract = StakeTokenContract::new(staking_pool_id, operator_id, None);

        contract.register_account();
        contract.withdraw_all();
    }
}
