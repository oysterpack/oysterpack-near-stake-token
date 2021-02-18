use crate::domain::RegisteredAccount;
use crate::errors::account_management::INSUFFICIENT_STORAGE_FEE;
use crate::errors::asserts::ATTACHED_DEPOSIT_IS_REQUIRED;
use crate::interface::{AccountManagement, AccountStorage, AccountStorageBalance, YoctoNear};
use crate::near::assert_yocto_near_attached;
use crate::*;
use near_sdk::{json_types::ValidAccountId, near_bindgen, Promise};

#[near_bindgen]
impl AccountStorage for Contract {
    /// To be compliant with the expected behavior for the Account Storage Standard API (NEP-145):
    /// - if overpayment is attached, then it is simply stored in the account storage escrow balance
    ///
    /// NOTE: We never want the function to panic.
    #[payable]
    fn storage_deposit(&mut self, account_id: Option<ValidAccountId>) -> AccountStorageBalance {
        assert!(env::attached_deposit() > 0, ATTACHED_DEPOSIT_IS_REQUIRED);

        let account_id = account_id.map_or_else(
            || env::predecessor_account_id(),
            |account_id| account_id.as_ref().to_string(),
        );
        match self.lookup_registered_account(&account_id) {
            // register the account
            None => {
                assert!(
                    env::attached_deposit() >= self.account_storage_fee().value(),
                    INSUFFICIENT_STORAGE_FEE,
                );
                let account = Account::new(env::attached_deposit().into());
                self.save_registered_account(&RegisteredAccount {
                    account,
                    id: Hash::from(&account_id),
                });
            }
            // deposit funds into account storage escrow
            Some(mut account) => {
                account
                    .storage_escrow
                    .credit(env::attached_deposit().into());
                self.save_registered_account(&account);
            }
        }
        // track total account storage escrow balance at contract level
        self.total_account_storage_escrow += env::attached_deposit().into();

        self._storage_balance_of(&account_id)
    }

    #[payable]
    fn storage_withdraw(&mut self, amount: Option<YoctoNear>) -> AccountStorageBalance {
        assert_yocto_near_attached();
        if let Some(amount) = amount.as_ref() {
            assert!(
                amount.value() > 0,
                "withdraw amount must be greater than zero"
            );
        }
        let mut account = self.predecessor_registered_account();

        let account_storage_balance = self.account_storage_balance(&account);
        let withdraw_amount = amount.unwrap_or(account_storage_balance.available.clone());
        assert!(
            withdraw_amount.value() <= account_storage_balance.available.value(),
            "ERR: account storage available balance is insufficient"
        );

        // update balances
        let withdraw_amount = withdraw_amount.into();
        account.storage_escrow.debit(withdraw_amount);
        self.save_registered_account(&account);
        self.total_account_storage_escrow -= withdraw_amount;

        // get updated account storage balance
        let account_storage_balance = self.account_storage_balance(&account);
        // transfer the withdrawal amount + the attached yoctoNEAR
        Promise::new(env::predecessor_account_id()).transfer(withdraw_amount.value() + 1);
        account_storage_balance
    }

    fn storage_minimum_balance(&self) -> YoctoNear {
        self.account_storage_fee()
    }

    fn storage_balance_of(&self, account_id: ValidAccountId) -> AccountStorageBalance {
        self._storage_balance_of(account_id.as_ref())
    }
}

impl Contract {
    /// accounts for changes in storage storage fees, i.e., if storage prices are lowered, then this
    /// will be reflected in the available balance.
    fn _storage_balance_of(&self, account_id: &str) -> AccountStorageBalance {
        match self.lookup_registered_account(account_id) {
            None => AccountStorageBalance::default(),
            Some(account) => self.account_storage_balance(&account),
        }
    }

    fn account_storage_balance(&self, account: &RegisteredAccount) -> AccountStorageBalance {
        AccountStorageBalance {
            total: account.storage_escrow.amount().into(),
            available: {
                let account_storage_fee = self.account_storage_fee().value();
                let storage_escrow_amount = account.storage_escrow.amount().value();
                if account_storage_fee > storage_escrow_amount {
                    0.into()
                } else {
                    (storage_escrow_amount - account_storage_fee).into()
                }
            },
        }
    }
}

#[cfg(test)]
mod test_storage_deposit {

    use super::*;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    fn account_id_not_registered_with_exact_deposit() {
        // Arrange
        let mut test_context = TestContext::new();
        let account_id = test_context.account_id.to_string();

        let mut context = test_context.context.clone();
        context.attached_deposit = test_context.storage_minimum_balance().value();
        testing_env!(context);

        // Act
        let balance = test_context.storage_deposit(Some(to_valid_account_id(&account_id)));

        // Assert
        assert_eq!(
            balance.total.value(),
            test_context.storage_minimum_balance().value()
        );
        assert_eq!(
            balance.total.value(),
            test_context.total_account_storage_escrow.value()
        );
        assert_eq!(balance.available.value(), 0);
        assert!(
            test_context.account_registered(to_valid_account_id(&account_id)),
            "the initial deposit should have registered the account"
        );
        assert_eq!(
            balance,
            test_context.storage_balance_of(to_valid_account_id(&account_id))
        );
    }

    #[test]
    fn account_id_not_registered_with_extra_deposit() {
        // Arrange
        let mut test_context = TestContext::new();
        let account_id = test_context.account_id.to_string();

        let mut context = test_context.context.clone();
        context.attached_deposit = test_context.storage_minimum_balance().value() * 3;
        testing_env!(context.clone());

        // Act
        let balance = test_context.storage_deposit(Some(to_valid_account_id(&account_id)));

        // Assert
        assert_eq!(balance.total.value(), context.attached_deposit);
        assert_eq!(
            balance.total.value(),
            test_context.total_account_storage_escrow.value()
        );
        assert_eq!(
            balance.available.value(),
            test_context.storage_minimum_balance().value() * 2
        );
        assert!(
            test_context.account_registered(to_valid_account_id(&account_id)),
            "the initial deposit should have registered the account"
        );
        assert_eq!(
            balance,
            test_context.storage_balance_of(to_valid_account_id(&account_id))
        );
    }

    #[test]
    #[should_panic(expected = "attached deposit is required")]
    fn account_id_not_registered_with_no_deposit() {
        // Arrange
        let mut test_context = TestContext::new();
        let account_id = test_context.account_id.to_string();

        // Act
        test_context.storage_deposit(Some(to_valid_account_id(&account_id)));
    }

    #[test]
    #[should_panic(expected = "sufficient deposit is required to pay for account storage fees")]
    fn account_id_not_registered_with_insufficient_deposit() {
        // Arrange
        let mut test_context = TestContext::new();
        let account_id = test_context.account_id.to_string();

        let mut context = test_context.context.clone();
        context.attached_deposit = test_context.storage_minimum_balance().value() - 1;
        testing_env!(context.clone());

        // Act
        test_context.storage_deposit(Some(to_valid_account_id(&account_id)));
    }

    //

    #[test]
    fn predecessor_account_id_not_registered_with_exact_deposit() {
        // Arrange
        let mut test_context = TestContext::new();
        let account_id = test_context.account_id.to_string();

        let mut context = test_context.context.clone();
        context.attached_deposit = test_context.storage_minimum_balance().value();
        testing_env!(context);

        // Act
        let balance = test_context.storage_deposit(None);

        // Assert
        assert_eq!(
            balance.total.value(),
            test_context.storage_minimum_balance().value()
        );
        assert_eq!(
            balance.total.value(),
            test_context.total_account_storage_escrow.value()
        );
        assert_eq!(balance.available.value(), 0);
        assert!(
            test_context.account_registered(to_valid_account_id(&account_id)),
            "the initial deposit should have registered the account"
        );
        assert_eq!(
            balance,
            test_context.storage_balance_of(to_valid_account_id(&account_id))
        );
    }

    #[test]
    fn predecessor_account_id_not_registered_with_extra_deposit() {
        // Arrange
        let mut test_context = TestContext::new();
        let account_id = test_context.account_id.to_string();

        let mut context = test_context.context.clone();
        context.attached_deposit = test_context.storage_minimum_balance().value() * 3;
        testing_env!(context.clone());

        // Act
        let balance = test_context.storage_deposit(None);

        // Assert
        assert_eq!(balance.total.value(), context.attached_deposit);
        assert_eq!(
            balance.total.value(),
            test_context.total_account_storage_escrow.value()
        );
        assert_eq!(
            balance.available.value(),
            test_context.storage_minimum_balance().value() * 2
        );
        assert!(
            test_context.account_registered(to_valid_account_id(&account_id)),
            "the initial deposit should have registered the account"
        );
        assert_eq!(
            balance,
            test_context.storage_balance_of(to_valid_account_id(&account_id))
        );
    }

    #[test]
    #[should_panic(expected = "attached deposit is required")]
    fn predecessor_account_id_not_registered_with_no_deposit() {
        // Arrange
        let mut test_context = TestContext::new();

        // Act
        test_context.storage_deposit(None);
    }

    #[test]
    #[should_panic(expected = "sufficient deposit is required to pay for account storage fees")]
    fn predecessor_account_id_not_registered_with_insufficient_deposit() {
        // Arrange
        let mut test_context = TestContext::new();

        let mut context = test_context.context.clone();
        context.attached_deposit = test_context.storage_minimum_balance().value() - 1;
        testing_env!(context.clone());

        // Act
        test_context.storage_deposit(None);
    }

    //

    #[test]
    #[should_panic(expected = "attached deposit is required")]
    fn account_id_registered_with_no_deposit() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let account_id = test_context.account_id.to_string();

        test_context.storage_deposit(Some(to_valid_account_id(&account_id)));
    }

    #[test]
    fn account_id_registered_with_deposit() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let account_id = test_context.account_id.to_string();

        let mut context = test_context.context.clone();
        context.attached_deposit = 1;
        testing_env!(context.clone());

        // Act
        let balance = test_context.storage_deposit(Some(to_valid_account_id(&account_id)));

        // Assert
        assert_eq!(balance.available.value(), context.attached_deposit);
        assert_eq!(
            test_context.total_account_storage_escrow.value(),
            test_context.account_storage_fee().value() + context.attached_deposit
        );
        assert_eq!(
            balance,
            test_context.storage_balance_of(to_valid_account_id(&account_id))
        );
    }

    //

    #[test]
    #[should_panic(expected = "attached deposit is required")]
    fn predecessor_account_id_registered_with_no_deposit() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();

        test_context.storage_deposit(None);
    }

    #[test]
    fn predecessor_account_id_registered_with_deposit() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();
        let account_id = test_context.account_id.to_string();

        let mut context = test_context.context.clone();
        context.attached_deposit = 1;
        testing_env!(context.clone());

        // Act
        let balance = test_context.storage_deposit(None);

        // Assert
        assert_eq!(balance.available.value(), context.attached_deposit);
        assert_eq!(
            test_context.total_account_storage_escrow.value(),
            test_context.account_storage_fee().value() + context.attached_deposit
        );
        assert_eq!(
            balance,
            test_context.storage_balance_of(to_valid_account_id(&account_id))
        );
    }
}

#[cfg(test)]
mod test_storage_withdraw {
    use super::*;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    #[should_panic(expected = "exactly 1 yoctoNEAR must be attached")]
    fn no_amount_no_attached_deposit() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();

        // Act
        test_context.storage_withdraw(None);
    }

    #[test]
    fn no_amount_specified_has_available_balance() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();

        let mut context = test_context.context.clone();
        context.attached_deposit = YOCTO;
        testing_env!(context);

        test_context.storage_deposit(None);

        let mut context = test_context.context.clone();
        context.attached_deposit = 1;
        testing_env!(context.clone());

        // Act
        let balance = test_context.storage_withdraw(None);

        // Assert
        assert_eq!(balance.total, test_context.storage_minimum_balance());
        assert_eq!(balance.available.value(), 0);
        let receipts = deserialize_receipts();
        assert_eq!(receipts.len(), 1);
        let receipt = &receipts[0];
        assert_eq!(receipt.receiver_id, context.predecessor_account_id);
        match &receipt.actions[0] {
            Action::Transfer { deposit } => assert_eq!(*deposit, YOCTO + 1),
            _ => panic!("expected transfer"),
        }
    }

    #[test]
    fn no_amount_specified_has_zero_available_balance() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();

        let mut context = test_context.context.clone();
        context.attached_deposit = 1;
        testing_env!(context.clone());

        // Act
        let balance = test_context.storage_withdraw(None);

        // Assert
        assert_eq!(balance.total, test_context.storage_minimum_balance());
        assert_eq!(balance.available.value(), 0);

        let receipts = deserialize_receipts();
        assert_eq!(receipts.len(), 1);
        let receipt = &receipts[0];
        assert_eq!(receipt.receiver_id, context.predecessor_account_id);
        match &receipt.actions[0] {
            Action::Transfer { deposit } => assert_eq!(*deposit, 1),
            _ => panic!("expected transfer"),
        }
    }

    #[test]
    #[should_panic(expected = "account is not registered")]
    fn no_amount_account_not_registered() {
        // Arrange
        let mut test_context = TestContext::new();

        let mut context = test_context.context.clone();
        context.attached_deposit = 1;
        testing_env!(context.clone());

        // Act
        test_context.storage_withdraw(None);
    }

    //

    #[test]
    #[should_panic(expected = "exactly 1 yoctoNEAR must be attached")]
    fn amount_specified_with_no_attached_deposit() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();

        // Act
        test_context.storage_withdraw(Some(100.into()));
    }

    #[test]
    fn amount_less_than_available_balance() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();

        let mut context = test_context.context.clone();
        context.attached_deposit = 300;
        testing_env!(context.clone());

        test_context.storage_deposit(None);

        let mut context = test_context.context.clone();
        context.attached_deposit = 1;
        testing_env!(context.clone());

        // Act
        let balance = test_context.storage_withdraw(Some(100.into()));
        assert_eq!(balance.available.value(), 200);

        let receipts = deserialize_receipts();
        assert_eq!(receipts.len(), 1);
        let receipt = &receipts[0];
        assert_eq!(receipt.receiver_id, context.predecessor_account_id);
        match &receipt.actions[0] {
            Action::Transfer { deposit } => assert_eq!(*deposit, 101),
            _ => panic!("expected transfer"),
        }
    }

    #[test]
    #[should_panic(expected = "ERR: account storage available balance is insufficient")]
    fn amount_more_than_available_balance() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();

        let mut context = test_context.context.clone();
        context.attached_deposit = 1;
        testing_env!(context.clone());

        // Act
        test_context.storage_withdraw(Some(100.into()));
    }

    #[test]
    fn amount_matches_available_balance() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();

        let mut context = test_context.context.clone();
        context.attached_deposit = 100;
        testing_env!(context.clone());

        test_context.storage_deposit(None);

        let mut context = test_context.context.clone();
        context.attached_deposit = 1;
        testing_env!(context.clone());

        // Act
        let balance = test_context.storage_withdraw(Some(100.into()));
        assert_eq!(balance.available.value(), 0);

        let receipts = deserialize_receipts();
        assert_eq!(receipts.len(), 1);
        let receipt = &receipts[0];
        assert_eq!(receipt.receiver_id, context.predecessor_account_id);
        match &receipt.actions[0] {
            Action::Transfer { deposit } => assert_eq!(*deposit, 101),
            _ => panic!("expected transfer"),
        }
    }

    #[test]
    #[should_panic(expected = "withdraw amount must be greater than zero")]
    fn amount_is_zero() {
        // Arrange
        let mut test_context = TestContext::with_registered_account();

        let mut context = test_context.context.clone();
        context.attached_deposit = 1;
        testing_env!(context.clone());

        // Act
        test_context.storage_withdraw(Some(0.into()));
    }
}

#[cfg(test)]
mod test_storage_minimum_balance {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn storage_min_balance_should_match_account_storage_fee() {
        let test_context = TestContext::new();

        assert_eq!(
            test_context.account_storage_fee(),
            test_context.storage_minimum_balance()
        );
    }
}
