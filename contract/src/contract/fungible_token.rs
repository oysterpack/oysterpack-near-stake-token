//required in order for near_bindgen macro to work outside of lib.rs
use crate::*;
use crate::{
    domain::{Gas, Vault, TGAS},
    errors::vault_fungible_token::{
        ACCOUNT_INSUFFICIENT_STAKE_FUNDS, RECEIVER_MUST_NOT_BE_SENDER, VAULT_ACCESS_DENIED,
        VAULT_DOES_NOT_EXIST, VAULT_INSUFFICIENT_FUNDS,
    },
    interface::{
        ext_self_finalize_transfer_callback, ext_self_resolve_vault_callback, ext_token_receiver,
        ext_transfer_call_recipient, fungible_token::events as fungible_token_events,
        FinalizeTransferCallback, FungibleToken, Metadata, ResolveVaultCallback, SimpleTransfer,
        TransferCall, TransferProtocol, VaultBasedTransfer, VaultId,
    },
    near::{assert_predecessor_is_self, log, NO_DEPOSIT},
};
use near_sdk::{
    env, json_types::ValidAccountId, json_types::U128, near_bindgen, AccountId, Promise,
};
use std::collections::HashMap;
use std::convert::TryFrom;

#[near_bindgen]
impl FungibleToken for StakeTokenContract {
    fn metadata(&self) -> Metadata {
        Metadata {
            name: "STAKE".to_string(),
            symbol: "STAKE".to_string(),
            reference: None,
            granularity: 1,
            supported_transfer_protocols: vec![
                TransferProtocol::simple(TGAS * 5),
                TransferProtocol::vault_transfer(
                    self.transfer_with_vault_gas()
                        + self.min_gas_for_vault_receiver()
                        + self.resolve_vault_gas(),
                ),
                TransferProtocol::transfer_and_notify(
                    self.transfer_call_gas()
                        + self.min_gas_for_transfer_call_receiver()
                        + self.finalize_ft_transfer_gas(),
                ),
            ],
        }
    }

    fn total_supply(&self) -> U128 {
        self.total_stake.amount().into()
    }

    fn balance(&self, account_id: ValidAccountId) -> U128 {
        let account = self.registered_account(account_id.as_ref());
        let account = self.apply_receipt_funds_for_view(&account);
        account
            .stake
            .map_or(0.into(), |balance| balance.amount().into())
    }
}

#[near_bindgen]
impl SimpleTransfer for StakeTokenContract {
    fn transfer(
        &mut self,
        recipient: ValidAccountId,
        amount: U128,
        headers: Option<HashMap<String, String>>,
    ) {
        let receiver_id: &str = recipient.as_ref();
        assert_receiver_is_not_sender(receiver_id);

        let mut sender = self.registered_account(&env::predecessor_account_id());
        let mut receiver = self.registered_account(receiver_id.as_ref());

        self.claim_receipt_funds(&mut sender);
        self.claim_receipt_funds(&mut receiver);

        let stake_amount = amount.into();
        assert!(
            sender.available_stake_balance().value() >= amount.0,
            ACCOUNT_INSUFFICIENT_STAKE_FUNDS
        );
        sender.apply_stake_debit(stake_amount);
        receiver.apply_stake_credit(stake_amount);

        self.save_registered_account(&sender);
        self.save_registered_account(&receiver);

        log(fungible_token_events::Transfer {
            from: &env::predecessor_account_id(),
            to: receiver_id,
            amount: amount.0,
            headers: headers.as_ref(),
        })
    }
}

#[near_bindgen]
impl VaultBasedTransfer for StakeTokenContract {
    fn transfer_with_vault(
        &mut self,
        recipient: ValidAccountId,
        amount: U128,
        headers: Option<HashMap<String, String>>,
    ) -> Promise {
        // the amount of gas required for the `resolve_vault` callback
        let resolve_vault_gas = self.resolve_vault_gas();

        // compute how much gas to supply to the receiver on the `on_receive_with_vault` cross contract call
        let on_receive_with_vault_gas = env::prepaid_gas()
            .saturating_sub(self.transfer_with_vault_gas().value() + resolve_vault_gas.value());

        if on_receive_with_vault_gas < self.min_gas_for_vault_receiver().value() {
            panic!(
                "Not enough gas attached. Attach at least {}",
                env::prepaid_gas()
                    + (self.min_gas_for_vault_receiver().value() - on_receive_with_vault_gas)
            );
        }

        let receiver_id: &str = recipient.as_ref();
        assert_receiver_is_not_sender(receiver_id);

        let mut sender = self.registered_account(&env::predecessor_account_id());
        self.claim_receipt_funds(&mut sender);

        // check that sender balance has sufficient funds
        let transfer_amount = amount.into();
        assert!(
            sender.available_stake_balance() >= transfer_amount,
            ACCOUNT_INSUFFICIENT_STAKE_FUNDS
        );
        sender.apply_stake_debit(transfer_amount);
        self.save_registered_account(&sender);

        // verifies that the receiver account is registered
        // - panics if the receiver account ID is not registered
        let mut receiver = self.registered_account(receiver_id);
        self.claim_receipt_funds(&mut receiver);

        // Creating a new vault
        *self.vault_id_sequence += 1;
        let vault = Vault(receiver.id, transfer_amount);
        self.vaults.insert(&self.vault_id_sequence, &vault);

        // Calling the receiver
        ext_token_receiver::on_receive_with_vault(
            env::predecessor_account_id(),
            transfer_amount.into(),
            self.vault_id_sequence.into(),
            headers,
            &receiver_id.to_string(),
            NO_DEPOSIT.value(),
            on_receive_with_vault_gas,
        )
        .then(ext_self_resolve_vault_callback::resolve_vault(
            self.vault_id_sequence.into(),
            env::predecessor_account_id(),
            &env::current_account_id(),
            NO_DEPOSIT.value(),
            resolve_vault_gas.value(),
        ))
    }

    fn withdraw_from_vault(&mut self, vault_id: VaultId, recipient: ValidAccountId, amount: U128) {
        let vault_id = vault_id.into();
        let mut vault = self.vaults.get(&vault_id).expect(VAULT_DOES_NOT_EXIST);

        let vault_owner = self.registered_account(&env::predecessor_account_id());
        if vault_owner.id != vault.owner_id_hash() {
            panic!(VAULT_ACCESS_DENIED);
        }

        // verifies that the receiver account is registered - panics if the account is not registered
        let mut receiver_account = self.registered_account(recipient.as_ref());

        // debit the vault
        let transfer_amount = amount.into();
        assert!(vault.balance() >= transfer_amount, VAULT_INSUFFICIENT_FUNDS);
        vault.debit(transfer_amount);
        self.vaults.insert(&vault_id, &vault);

        // credit the receiver account
        receiver_account.apply_stake_credit(transfer_amount);
        self.save_registered_account(&receiver_account);
    }
}

#[near_bindgen]
impl ResolveVaultCallback for StakeTokenContract {
    fn resolve_vault(&mut self, vault_id: VaultId, sender_id: AccountId) -> U128 {
        assert_predecessor_is_self();

        let vault = self
            .vaults
            .remove(&vault_id.into())
            .expect(VAULT_DOES_NOT_EXIST);
        if vault.balance().value() > 0 {
            let mut account = self.registered_account(&sender_id);
            account.apply_stake_credit(vault.balance());
            self.save_registered_account(&account);
        }
        vault.balance().into()
    }
}

#[near_bindgen]
impl TransferCall for StakeTokenContract {
    fn transfer_call(
        &mut self,
        recipient: ValidAccountId,
        amount: U128,
        headers: Option<HashMap<String, String>>,
    ) -> Promise {
        // the amount of gas required for the `resolve_vault` callback
        let finalize_ft_transfer_gas = self.finalize_ft_transfer_gas();

        // compute how much gas to supply to the receiver on the `on_receive_with_vault` cross contract call
        let transfer_call_receiver_gas = env::prepaid_gas()
            .saturating_sub(self.transfer_call_gas().value() + finalize_ft_transfer_gas.value());

        if transfer_call_receiver_gas < self.min_gas_for_transfer_call_receiver().value() {
            panic!(
                "Not enough gas attached. Attach at least {}",
                env::prepaid_gas()
                    + (self.min_gas_for_transfer_call_receiver().value()
                        - transfer_call_receiver_gas)
            );
        }

        let receiver_id: &str = recipient.as_ref();
        assert_receiver_is_not_sender(receiver_id);

        let mut sender = self.registered_account(&env::predecessor_account_id());
        self.claim_receipt_funds(&mut sender);

        // check that sender balance has sufficient funds
        let transfer_amount = amount.into();
        assert!(
            sender.available_stake_balance() >= transfer_amount,
            ACCOUNT_INSUFFICIENT_STAKE_FUNDS
        );
        sender.apply_stake_debit(transfer_amount);
        self.save_registered_account(&sender);

        // verifies that the receiver account is registered
        // - panics if the receiver account ID is not registered
        let mut receiver = self.registered_account(receiver_id);
        self.claim_receipt_funds(&mut receiver);
        receiver.apply_stake_credit(transfer_amount);
        receiver.lock_stake(transfer_amount);
        self.save_registered_account(&receiver);

        // Calling the receiver
        ext_transfer_call_recipient::on_ft_receive(
            ValidAccountId::try_from(env::predecessor_account_id()).unwrap(),
            transfer_amount.into(),
            headers,
            &receiver_id.to_string(),
            NO_DEPOSIT.value(),
            transfer_call_receiver_gas,
        )
        .then(ext_self_finalize_transfer_callback::finalize_ft_transfer(
            env::predecessor_account_id(),
            receiver_id.to_string(),
            transfer_amount.into(),
            &env::current_account_id(),
            NO_DEPOSIT.value(),
            finalize_ft_transfer_gas.value(),
        ))
    }
}

#[near_bindgen]
impl FinalizeTransferCallback for StakeTokenContract {
    fn finalize_ft_transfer(&mut self, sender: AccountId, recipient: AccountId, amount: U128) {
        assert_predecessor_is_self();

        if self.promise_result_succeeded() {
            // unlock the balance on the recipient account
            let mut receiver = self.registered_account(&recipient);
            receiver.unlock_stake(amount.into());
            self.save_registered_account(&receiver);
        } else {
            // rollback the transfer
            log("`transfer_call` failed: rolling back token transfer");

            let mut receiver = self.registered_account(&recipient);
            receiver.unlock_stake(amount.into());
            receiver.apply_stake_debit(amount.into());
            self.save_registered_account(&receiver);

            let mut sender = self.registered_account(&sender);
            sender.apply_stake_credit(amount.into());
            self.save_registered_account(&sender);
        }
    }
}

impl StakeTokenContract {
    fn resolve_vault_gas(&self) -> Gas {
        self.config
            .gas_config()
            .vault_fungible_token()
            .resolve_vault()
    }

    fn transfer_with_vault_gas(&self) -> Gas {
        self.config
            .gas_config()
            .vault_fungible_token()
            .transfer_with_vault()
    }

    fn min_gas_for_vault_receiver(&self) -> Gas {
        self.config
            .gas_config()
            .vault_fungible_token()
            .min_gas_for_receiver()
    }

    fn finalize_ft_transfer_gas(&self) -> Gas {
        self.config
            .gas_config()
            .transfer_call_fungible_token()
            .finalize_ft_transfer()
    }

    fn transfer_call_gas(&self) -> Gas {
        self.config
            .gas_config()
            .transfer_call_fungible_token()
            .transfer_call()
    }

    fn min_gas_for_transfer_call_receiver(&self) -> Gas {
        self.config
            .gas_config()
            .transfer_call_fungible_token()
            .min_gas_for_receiver()
    }
}

fn assert_receiver_is_not_sender(receiver_id: &str) {
    assert_ne!(
        &env::predecessor_account_id(),
        receiver_id,
        "{}",
        RECEIVER_MUST_NOT_BE_SENDER
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interface::StakingService;
    use crate::{interface::AccountManagement, near::YOCTO, test_utils::*, Hash};
    use near_sdk::{
        json_types::U128, serde::Deserialize, serde_json, testing_env, MockedBlockchain,
    };
    use std::convert::TryFrom;

    /// Given the sender and receiver accounts are registered
    /// And the sender has STAKE funds to transfer
    /// When the sender transfers STAKE to the receiver
    /// Then the sender account will be debited, and the receiver account will be credited
    #[test]
    fn transfer_success() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        assert_eq!(account.available_stake_balance(), (100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        contract.transfer(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (10 * YOCTO).into(),
            None,
        );

        assert_eq!(
            contract.balance(ValidAccountId::try_from(sender_account_id).unwrap()),
            (90 * YOCTO).into()
        );
        assert_eq!(
            contract.balance(ValidAccountId::try_from(receiver_account_id).unwrap()),
            (10 * YOCTO).into()
        );
        assert_eq!(contract.total_supply(), (100 * YOCTO).into());
    }

    #[test]
    #[should_panic(expected = "account STAKE balance is to low to fulfill request")]
    fn transfer_sender_balance_too_low() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        contract.transfer(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (110 * YOCTO).into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "receiver account must not be the sender")]
    fn transfer_receiver_is_sender() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        contract.transfer(
            ValidAccountId::try_from(account_id).unwrap(),
            YOCTO.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "account is not registered: joe.near")]
    fn transfer_sender_not_registered() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = "joe.near".to_string();
        testing_env!(context.clone());
        contract.transfer(
            ValidAccountId::try_from(account_id).unwrap(),
            YOCTO.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "account is not registered: joe.near")]
    fn transfer_receiver_not_registered() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        contract.transfer(
            ValidAccountId::try_from("joe.near").unwrap(),
            YOCTO.into(),
            None,
        );
    }

    /// Given the sender and receiver accounts are registered
    /// And the sender has STAKE tokens to transfer
    /// When the sender transfers tokens using a vault
    /// Then the transaction will generate 2 receipts
    ///   1. func call `on_receive_with_vault` on receiver account
    ///   2. func callback `resolve_vault`
    /// And the vault is created for the receiver holding the transfer amount
    /// And the sender account is debited the transfer amount
    #[test]
    fn transfer_with_vault_success() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = 10 * YOCTO;
        let mut payload = HashMap::new();
        payload.insert("msg".to_string(), "Happy New Year".to_string());

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
            Some(payload.clone()),
        );
        let vault = contract.vaults.get(&contract.vault_id_sequence).unwrap();
        assert_eq!(vault.balance(), transfer_amount.into());
        assert_eq!(vault.owner_id_hash(), Hash::from(receiver_account_id));
        assert_eq!(
            contract.balance(ValidAccountId::try_from(sender_account_id).unwrap()),
            (90 * YOCTO).into()
        );

        let receipts = deserialize_receipts(&env::created_receipts());
        println!("{:#?}", receipts);
        {
            let receipt = &receipts[0];
            assert_eq!(receipt.receiver_id, receiver_account_id);
            match receipt.actions.first().unwrap() {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "on_receive_with_vault");
                    let args: TransferWithVaultArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(args.vault_id, contract.vault_id_sequence.value().into());
                    assert_eq!(args.headers.unwrap(), payload);
                }
                _ => panic!("invalid action type"),
            }
        }
        {
            let receipt = &receipts[1];
            assert_eq!(receipt.receiver_id, context.current_account_id);
            match receipt.actions.first().unwrap() {
                Action::FunctionCall {
                    method_name, args, ..
                } => {
                    assert_eq!(method_name, "resolve_vault");
                    let args: ResolveVaultArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(args.vault_id, contract.vault_id_sequence.value().into());
                    assert_eq!(args.sender_id, sender_account_id);
                }
                _ => panic!("invalid action type"),
            }
        }
    }

    #[test]
    #[should_panic(expected = "account STAKE balance is to low to fulfill request")]
    fn transfer_with_vault_sender_balance_too_low() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (110 * YOCTO).into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "receiver account must not be the sender")]
    fn transfer_with_vault_receiver_is_sender() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        contract.transfer_with_vault(
            ValidAccountId::try_from(account_id).unwrap(),
            YOCTO.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "account is not registered: joe.near")]
    fn transfer_with_vault_sender_not_registered() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = "joe.near".to_string();
        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(account_id).unwrap(),
            YOCTO.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "account is not registered: joe.near")]
    fn transfer_with_vault_receiver_not_registered() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        let mut account = contract.registered_account(account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        contract.transfer_with_vault(
            ValidAccountId::try_from("joe.near").unwrap(),
            YOCTO.into(),
            None,
        );
    }

    /// Given the sender has done a transfer with vault to the receiver
    /// When receiver tries to withdraw funds from the vault
    /// Then the funds are transferred from the vault to the receiver account
    #[test]
    fn withdraw_from_vault_success() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = 10 * YOCTO;
        let mut payload = HashMap::new();
        payload.insert("msg".to_string(), "Happy New Year".to_string());

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
            Some(payload.clone()),
        );

        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
        );

        let vault = contract.vaults.get(&contract.vault_id_sequence).unwrap();
        assert_eq!(vault.balance().value(), 0);
        assert_eq!(
            contract
                .balance(ValidAccountId::try_from(receiver_account_id).unwrap())
                .0,
            transfer_amount
        );
    }

    #[test]
    #[should_panic(expected = "Not enough gas attached. Attach at least")]
    fn withdraw_from_vault_insufficient_gas() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = 10 * YOCTO;

        context.prepaid_gas = contract
            .config
            .gas_config()
            .vault_fungible_token()
            .min_gas_for_receiver()
            .value()
            - 1;
        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "account is not registered: unknown.near")]
    fn withdraw_from_vault_receiver_account_not_registered() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = 10 * YOCTO;

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
            None,
        );

        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from("unknown.near").unwrap(),
            transfer_amount.into(),
        );
    }

    #[test]
    #[should_panic(expected = "vault access is denied")]
    fn withdraw_from_vault_access_denied() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = 10 * YOCTO;

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
            None,
        );

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
        );
    }

    #[test]
    #[should_panic(expected = "vault balance is too low to fulfill withdrawal request")]
    fn withdraw_from_vault_vault_balance_too_low() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = 10 * YOCTO;

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
            None,
        );

        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (transfer_amount + 1).into(),
        );
    }

    #[test]
    #[should_panic(expected = "vault does not exist")]
    fn withdraw_from_vault_vault_does_not_exist() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = 10 * YOCTO;

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
            None,
        );

        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            10.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (transfer_amount + 1).into(),
        );
    }

    #[test]
    fn resolve_vault_success() {
        let sender_account_id = "sender.near";
        let receiver_account_id = "receiver.near";

        let mut context = new_context(receiver_account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(None, contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let mut account = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = 10 * YOCTO;

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.into(),
            None,
        );

        let withdrawal_amount = 4 * YOCTO;
        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            withdrawal_amount.into(),
        );

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());
        contract.resolve_vault(
            contract.vault_id_sequence.into(),
            sender_account_id.to_string(),
        );

        assert!(contract.vaults.get(&contract.vault_id_sequence).is_none());
        assert_eq!(
            contract.balance(ValidAccountId::try_from(sender_account_id).unwrap()),
            (96 * YOCTO).into()
        );
        assert_eq!(
            contract
                .balance(ValidAccountId::try_from(receiver_account_id).unwrap())
                .0,
            withdrawal_amount
        );
    }

    #[test]
    fn get_balance_with_unclaimed_receipts() {
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
        contract.save_registered_account(&account);

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(
            contract.balance(ValidAccountId::try_from(account_id).unwrap()),
            (10 * YOCTO).into()
        );
    }

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct TransferWithVaultArgs {
        vault_id: U128,
        headers: Option<HashMap<String, String>>,
    }

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct ResolveVaultArgs {
        vault_id: U128,
        sender_id: String,
    }
}
