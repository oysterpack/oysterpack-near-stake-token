use crate::interface::{AccountManagement, ContractFinancials, ContractOwner, YoctoNear};
//required in order for near_bindgen macro to work outside of lib.rs
use crate::errors::contract_owner::{
    INSUFFICIENT_FUNDS_FOR_OWNER_STAKING, INSUFFICIENT_FUNDS_FOR_OWNER_WITHDRAWAL,
    TRANSFER_TO_NON_REGISTERED_ACCOUNT,
};
use crate::interface::contract_owner::events::OwnershipTransferred;
use crate::near::log;
use crate::*;
use near_sdk::{json_types::ValidAccountId, near_bindgen, Promise};

#[near_bindgen]
impl ContractOwner for Contract {
    fn owner_id(&self) -> AccountId {
        self.owner_id.clone()
    }

    fn transfer_ownership(&mut self, new_owner: ValidAccountId) {
        self.assert_predecessor_is_owner();
        assert!(
            self.account_registered(new_owner.clone()),
            TRANSFER_TO_NON_REGISTERED_ACCOUNT,
        );

        let previous_owner = self.owner_id.clone();
        self.owner_id = new_owner.into();
        self.operator_id = self.owner_id.clone();

        log(OwnershipTransferred {
            from: &previous_owner,
            to: &self.owner_id,
        });
    }

    fn set_operator_id(&mut self, account_id: ValidAccountId) {
        self.assert_predecessor_is_owner();
        assert!(
            self.account_registered(account_id.clone()),
            TRANSFER_TO_NON_REGISTERED_ACCOUNT,
        );

        self.operator_id = account_id.into();
    }

    fn stake_all_owner_balance(&mut self) -> YoctoNear {
        self.assert_predecessor_is_owner();
        let mut account = self.registered_account(&self.owner_id);
        let balances = self.balances();
        let owner_available_balance = balances.contract_owner_available_balance;
        assert!(owner_available_balance.value() > 0, "owner balance is zero");
        self.deposit_near_for_account_to_stake(
            &mut account,
            owner_available_balance.value().into(),
        );
        self.save_registered_account(&account);
        owner_available_balance
    }

    fn stake_owner_balance(&mut self, amount: YoctoNear) {
        self.assert_predecessor_is_owner();
        let mut account = self.registered_account(&self.owner_id);
        let owner_available_balance = self.balances().contract_owner_available_balance;
        assert!(
            owner_available_balance.value() >= amount.value(),
            INSUFFICIENT_FUNDS_FOR_OWNER_STAKING
        );
        self.deposit_near_for_account_to_stake(&mut account, amount.into());
        self.save_registered_account(&account);
    }

    fn withdraw_all_owner_balance(&mut self) -> YoctoNear {
        self.assert_predecessor_is_owner();
        let owner_available_balance = self.balances().contract_owner_available_balance;
        Promise::new(self.owner_id.clone()).transfer(owner_available_balance.value());
        owner_available_balance
    }

    fn withdraw_owner_balance(&mut self, amount: YoctoNear) {
        self.assert_predecessor_is_owner();
        let owner_available_balance = self.balances().contract_owner_available_balance;
        assert!(
            owner_available_balance.value() >= amount.value(),
            INSUFFICIENT_FUNDS_FOR_OWNER_WITHDRAWAL
        );
        Promise::new(self.owner_id.clone()).transfer(amount.value());
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interface::ContractFinancials;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};
    use std::convert::TryFrom;

    #[test]
    fn transfer_ownership_success() {
        let mut ctx = TestContext::with_registered_account();
        let mut context = ctx.context.clone();
        let contract = &mut ctx.contract;

        let new_owner = ctx.account_id;

        context.predecessor_account_id = contract.owner_id.clone();
        testing_env!(context.clone());

        contract.transfer_ownership(ValidAccountId::try_from(new_owner).unwrap());
        assert_eq!(&contract.owner_id, new_owner);
        assert_eq!(contract.operator_id, new_owner);
    }

    #[test]
    fn set_operator_id() {
        let mut ctx = TestContext::with_registered_account();
        let mut context = ctx.context.clone();
        let contract = &mut ctx.contract;

        context.predecessor_account_id = contract.owner_id.clone();
        testing_env!(context.clone());

        contract.set_operator_id(ValidAccountId::try_from(ctx.account_id).unwrap());
        assert_eq!(contract.operator_id, ctx.account_id);
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn set_operator_id_invoked_by_non_owner() {
        let mut ctx = TestContext::with_registered_account();
        let mut context = ctx.context.clone();
        let contract = &mut ctx.contract;

        context.predecessor_account_id = ctx.account_id.to_string();
        testing_env!(context.clone());

        contract.set_operator_id(ValidAccountId::try_from(ctx.account_id).unwrap());
        assert_eq!(contract.operator_id, ctx.account_id);
    }

    #[test]
    #[should_panic(expected = "contract ownership can only be transferred to a registered account")]
    fn transfer_ownership_to_non_registered_account() {
        let mut ctx = TestContext::new();
        let mut context = ctx.context.clone();
        let contract = &mut ctx.contract;

        context.predecessor_account_id = contract.owner_id.clone();
        testing_env!(context.clone());
        contract.transfer_ownership(ValidAccountId::try_from(ctx.account_id).unwrap());
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn transfer_ownership_from_non_owner() {
        let mut ctx = TestContext::with_registered_account();
        let contract = &mut ctx.contract;

        testing_env!(ctx.context.clone());
        contract.transfer_ownership(ValidAccountId::try_from(ctx.account_id).unwrap());
    }

    #[test]
    fn withdraw_all_owner_balance_success() {
        let mut test_context = TestContext::new();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        let owner_available_balance = contract.balances().contract_owner_available_balance;

        context.predecessor_account_id = contract.owner_id();
        testing_env!(context.clone());
        contract.withdraw_all_owner_balance();
        let receipts = deserialize_receipts();
        assert_eq!(receipts.len(), 1);
        let receipt = receipts.first().unwrap();
        println!("{:#?}", receipt);
        assert_eq!(receipt.receiver_id, contract.owner_id());
        if let Action::Transfer { deposit } = receipt.actions.first().unwrap() {
            assert_eq!(owner_available_balance.value(), *deposit);
        } else {
            panic!("expected transfer action");
        }
    }

    #[test]
    fn withdraw_owner_balance_success() {
        let mut test_context = TestContext::new();
        let mut context = test_context.context.clone();
        let contract = &mut test_context.contract;

        context.predecessor_account_id = contract.owner_id();
        testing_env!(context.clone());
        contract.withdraw_owner_balance(YOCTO.into());
        let receipts = deserialize_receipts();
        assert_eq!(receipts.len(), 1);
        let receipt = receipts.first().unwrap();
        println!("{:#?}", receipt);
        assert_eq!(receipt.receiver_id, contract.owner_id());
        if let Action::Transfer { deposit } = receipt.actions.first().unwrap() {
            assert_eq!(YOCTO, *deposit);
        } else {
            panic!("expected transfer action");
        }
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn withdraw_all_owner_balance_called_by_non_owner() {
        let mut test_context = TestContext::new();
        test_context.contract.withdraw_all_owner_balance();
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn withdraw_owner_balance_called_by_non_owner() {
        let mut context = TestContext::new();
        let contract = &mut context.contract;
        let mut vm_ctx = context.context.clone();
        vm_ctx.predecessor_account_id = "non-owner.near".to_string();
        testing_env!(vm_ctx);
        contract.withdraw_owner_balance(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn stake_owner_balance_called_by_non_owner() {
        let mut context = TestContext::new();
        let contract = &mut context.contract;
        let mut vm_ctx = context.context.clone();
        vm_ctx.predecessor_account_id = "non-owner.near".to_string();
        testing_env!(vm_ctx);
        contract.stake_owner_balance(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn stake_all_owner_balance_called_by_non_owner() {
        let mut context = TestContext::new();
        let contract = &mut context.contract;
        let mut vm_ctx = context.context.clone();
        vm_ctx.predecessor_account_id = "non-owner.near".to_string();
        testing_env!(vm_ctx);
        contract.stake_all_owner_balance();
    }

    #[test]
    fn stake_all_owner_balance_success() {
        let mut context = TestContext::with_registered_account();
        context.register_owner();
        let contract = &mut context.contract;

        contract.stake_all_owner_balance();
        let account = contract
            .lookup_account(ValidAccountId::try_from(contract.owner_id.as_str()).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_some());
    }

    #[test]
    fn stake_owner_balance_success() {
        let mut context = TestContext::with_registered_account();
        context.register_owner();
        let contract = &mut context.contract;

        contract.stake_owner_balance(YOCTO.into());
        let account = contract
            .lookup_account(ValidAccountId::try_from(contract.owner_id.as_str()).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_some());
    }
}
