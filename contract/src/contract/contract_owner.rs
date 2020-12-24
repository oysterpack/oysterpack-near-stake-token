use crate::interface::{AccountManagement, ContractOwner, YoctoNear};
//required in order for near_bindgen macro to work outside of lib.rs
use crate::errors::contract_owner::{
    INSUFFICIENT_FUNDS_FOR_OWNER_STAKING, INSUFFICIENT_FUNDS_FOR_OWNER_WITHDRAWAL,
};
use crate::near::{log, YOCTO};
use crate::*;
use crate::{
    errors::contract_owner::ACCOUNT_VALIDATION_NEAR_TRANSFER_FAILED,
    interface::ext_contract_owner_callbacks,
    near::{assert_predecessor_is_self, NO_DEPOSIT},
};
use near_sdk::{json_types::ValidAccountId, near_bindgen, Promise};

#[near_bindgen]
impl ContractOwner for StakeTokenContract {
    fn owner_id(&self) -> AccountId {
        self.owner_id.clone()
    }

    fn owner_balance(&self) -> YoctoNear {
        let total_customer_accounts_unstaked_balance = self.total_near.amount().value();
        let customer_batched_stake_deposits = self
            .stake_batch
            .map_or(0, |batch| batch.balance().amount().value())
            + self
                .next_stake_batch
                .map_or(0, |batch| batch.balance().amount().value());
        let total_account_storage_escrow =
            self.total_registered_accounts().0 * self.account_storage_fee().value();

        let contract_storage_usage_cost =
            env::storage_usage() as u128 * self.config.storage_cost_per_byte().value();

        let owner_balance = env::account_balance()
            - total_customer_accounts_unstaked_balance
            - customer_batched_stake_deposits
            - total_account_storage_escrow
            - contract_storage_usage_cost;

        if owner_balance > YOCTO {
            (owner_balance - YOCTO).into()
        } else {
            0.into()
        }
    }

    fn transfer_ownership(&self, new_owner: ValidAccountId) -> Promise {
        self.assert_predecessor_is_owner();
        Promise::new(new_owner.as_ref().to_string())
            .transfer(1)
            .then(self.invoke_finalize_ownership_transfer(new_owner.as_ref().to_string()))
    }

    fn stake_all_owner_balance(&mut self) -> YoctoNear {
        self.assert_predecessor_is_owner();
        let (mut account, account_id_hash) = self.registered_account(&self.owner_id);
        let owner_balance = self.owner_balance();
        assert!(owner_balance.value() > 0, "owner balance is zero");
        self.deposit_near_for_account_to_stake(&mut account, owner_balance.value().into());
        self.save_account(&account_id_hash, &account);
        owner_balance
    }

    fn stake_owner_balance(&mut self, amount: YoctoNear) {
        self.assert_predecessor_is_owner();
        let (mut account, account_id_hash) = self.registered_account(&self.owner_id);
        let owner_balance = self.owner_balance();
        assert!(
            owner_balance.value() >= amount.value(),
            INSUFFICIENT_FUNDS_FOR_OWNER_STAKING
        );
        self.deposit_near_for_account_to_stake(&mut account, amount.into());
        self.save_account(&account_id_hash, &account);
    }

    fn withdraw_all_owner_balance(&mut self) -> YoctoNear {
        self.assert_predecessor_is_owner();
        let owner_balance = self.owner_balance();
        Promise::new(self.owner_id.clone()).transfer(owner_balance.value());
        owner_balance
    }

    fn withdraw_owner_balance(&mut self, amount: YoctoNear) {
        self.assert_predecessor_is_owner();
        let owner_balance = self.owner_balance();
        assert!(
            owner_balance.value() >= amount.value(),
            INSUFFICIENT_FUNDS_FOR_OWNER_WITHDRAWAL
        );
        Promise::new(self.owner_id.clone()).transfer(amount.value());
    }
}

/// callbacks
#[near_bindgen]
impl StakeTokenContract {
    pub fn finalize_transfer_ownership(&mut self, new_owner: AccountId) {
        assert_predecessor_is_self();

        assert!(
            self.promise_result_succeeded(),
            ACCOUNT_VALIDATION_NEAR_TRANSFER_FAILED
        );

        self.owner_id = new_owner.into();

        log(format!(
            "contract ownership has been transferred to {}",
            self.owner_id
        ));
    }
}

impl StakeTokenContract {
    fn invoke_finalize_ownership_transfer(&self, new_owner: AccountId) -> Promise {
        ext_contract_owner_callbacks::finalize_transfer_ownership(
            new_owner,
            &env::current_account_id(),
            NO_DEPOSIT.into(),
            self.config
                .gas_config()
                .callbacks()
                .finalize_ownership_transfer()
                .value(),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{serde::Deserialize, serde_json, testing_env, MockedBlockchain};
    use std::convert::TryFrom;

    #[test]
    fn owner_balance_has_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        context.attached_deposit = contract.account_storage_fee().value();
        testing_env!(context.clone());
        contract.register_account();

        assert_eq!(
            env::account_balance(),
            (100 * YOCTO) + contract.account_storage_fee().value()
        );

        testing_env!(context.clone());
        assert_eq!(contract.owner_balance(), (59 * YOCTO).into());

        contract.total_near.credit((50 * YOCTO).into());
        testing_env!(context.clone());
        assert_eq!(contract.owner_balance(), (9 * YOCTO).into());
    }

    #[test]
    fn owner_balance_has_funds_with_pending_stake_batches() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        context.attached_deposit = contract.account_storage_fee().value();
        testing_env!(context.clone());
        contract.register_account();

        *contract.batch_id_sequence += 1;
        contract.stake_batch = Some(domain::StakeBatch::new(
            contract.batch_id_sequence,
            YOCTO.into(),
        ));
        *contract.batch_id_sequence += 1;
        contract.next_stake_batch = Some(domain::StakeBatch::new(
            contract.batch_id_sequence,
            (2 * YOCTO).into(),
        ));

        testing_env!(context.clone());
        assert_eq!(contract.owner_balance(), (56 * YOCTO).into());

        contract.total_near.credit((10 * YOCTO).into());
        testing_env!(context.clone());
        assert_eq!(contract.owner_balance(), (46 * YOCTO).into());
    }

    #[test]
    fn transfer_ownership_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(contract_settings);

        context.predecessor_account_id = contract.owner_id.clone();
        testing_env!(context.clone());
        contract.transfer_ownership(ValidAccountId::try_from(account_id).unwrap());
        let receipts = deserialize_receipts(&env::created_receipts());
        assert_eq!(receipts.len(), 2);
        println!("{:#?}", receipts);
        {
            let receipt = receipts.first().unwrap();
            assert_eq!(receipt.receiver_id, account_id);
            match receipt.actions.first() {
                Some(Action::Transfer { deposit }) => assert_eq!(*deposit, 1),
                _ => panic!("transfer action i expected0"),
            }
        }

        let receipt = &receipts[1];
        assert_eq!(receipt.receiver_id, env::current_account_id());
        match receipt.actions.first() {
            Some(Action::FunctionCall {
                method_name, args, ..
            }) => {
                assert_eq!(method_name, "finalize_transfer_ownership");
                let args: TransferOwnershipArgs = serde_json::from_str(args).unwrap();
                assert_eq!(args.new_owner, account_id);
            }
            _ => panic!("transfer action i expected0"),
        }
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn transfer_ownership_from_non_owner() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(contract_settings);

        testing_env!(context.clone());
        contract.transfer_ownership(ValidAccountId::try_from(account_id).unwrap());
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed internally")]
    fn finalize_transfer_ownership_called_by_non_self() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.finalize_transfer_ownership("new-owner.testnet".to_string());
    }

    #[test]
    fn finalize_transfer_ownership_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());
        contract.finalize_transfer_ownership("new-owner.testnet".to_string());
        assert_eq!(contract.owner_id, "new-owner.testnet");
    }

    #[test]
    fn withdraw_all_owner_balance_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        let owner_balance = contract.owner_balance();

        context.predecessor_account_id = contract.owner_id();
        testing_env!(context.clone());
        contract.withdraw_all_owner_balance();
        let receipts = deserialize_receipts(&env::created_receipts());
        assert_eq!(receipts.len(), 1);
        let receipt = receipts.first().unwrap();
        println!("{:#?}", receipt);
        assert_eq!(receipt.receiver_id, contract.owner_id());
        if let Action::Transfer { deposit } = receipt.actions.first().unwrap() {
            assert_eq!(owner_balance.value(), *deposit);
        } else {
            panic!("expected transfer action");
        }
    }

    #[test]
    fn withdraw_owner_balance_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        let owner_balance = contract.owner_balance();

        context.predecessor_account_id = contract.owner_id();
        testing_env!(context.clone());
        contract.withdraw_owner_balance(YOCTO.into());
        let receipts = deserialize_receipts(&env::created_receipts());
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
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.withdraw_all_owner_balance();
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn withdraw_owner_balance_called_by_non_owner() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.withdraw_owner_balance(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn stake_owner_balance_called_by_non_owner() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.stake_owner_balance(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "contract call is only allowed by the contract owner")]
    fn stake_all_owner_balance_called_by_non_owner() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.stake_all_owner_balance();
    }

    #[test]
    fn stake_all_owner_balance_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        let owner_balance = contract.owner_balance();

        context.attached_deposit = YOCTO;
        context.predecessor_account_id = contract.owner_id();
        testing_env!(context.clone());
        contract.register_account();
        contract.stake_all_owner_balance();
        let account = contract
            .lookup_account(ValidAccountId::try_from(contract.owner_id.as_str()).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_some());
    }

    #[test]
    fn stake_owner_balance_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.account_balance = 100 * YOCTO;
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        let owner_balance = contract.owner_balance();

        context.attached_deposit = YOCTO;
        context.predecessor_account_id = contract.owner_id();
        testing_env!(context.clone());
        contract.register_account();
        contract.stake_owner_balance(YOCTO.into());
        let account = contract
            .lookup_account(ValidAccountId::try_from(contract.owner_id.as_str()).unwrap())
            .unwrap();
        assert!(account.stake_batch.is_some());
    }

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct TransferOwnershipArgs {
        new_owner: String,
    }
}
