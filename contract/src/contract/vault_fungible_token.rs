use crate::*;
use crate::{
    domain::Vault,
    errors::vault_fungible_token::{
        ACCOUNT_INSUFFICIENT_STAKE_FUNDS, RECEIVER_MUST_NOT_BE_SENDER, VAULT_ACCESS_DENIED,
        VAULT_DOES_NOT_EXIST, VAULT_INSUFFICIENT_FUNDS,
    },
    interface::{
        ext_self, ext_token_receiver, ResolveVaultCallback, VaultFungibleToken, VaultId, YoctoStake,
    },
    near::{assert_predecessor_is_self, NO_DEPOSIT},
    StakeTokenContract,
};

use near_sdk::{env, json_types::ValidAccountId, near_bindgen, AccountId, Promise};

#[near_bindgen]
impl VaultFungibleToken for StakeTokenContract {
    fn transfer(&mut self, receiver_id: ValidAccountId, amount: YoctoStake) {
        let receiver_id: &str = receiver_id.as_ref();
        assert_receiver_is_not_sender(receiver_id);

        let (mut sender, sender_account_id) =
            self.registered_account(&env::predecessor_account_id());
        let (mut receiver, receiver_account_id) = self.registered_account(receiver_id.as_ref());

        let stake_amount = amount.into();
        sender.apply_stake_debit(stake_amount);
        receiver.apply_stake_credit(stake_amount);

        self.save_account(&sender_account_id, &sender);
        self.save_account(&receiver_account_id, &receiver);
    }

    fn transfer_with_vault(
        &mut self,
        receiver_id: ValidAccountId,
        amount: YoctoStake,
        payload: String,
    ) -> Promise {
        let transfer_with_vault_gas = self
            .config
            .gas_config()
            .vault_fungible_token()
            .transfer_with_vault();

        let resolve_vault_gas = self
            .config
            .gas_config()
            .vault_fungible_token()
            .resolve_vault();

        let gas_to_receiver = env::prepaid_gas()
            .saturating_sub(transfer_with_vault_gas.value() + resolve_vault_gas.value());

        if gas_to_receiver
            < self
                .config
                .gas_config()
                .vault_fungible_token()
                .min_gas_for_receiver()
                .value()
        {
            panic!(
                "Not enough gas attached. Attach at least {}",
                gas_to_receiver
            );
        }

        let receiver_id: &str = receiver_id.as_ref();
        assert_receiver_is_not_sender(receiver_id);

        let (mut sender, sender_account_id) =
            self.registered_account(&env::predecessor_account_id());

        // check that sender balance has sufficient funds
        let sender_balance = sender.stake.expect(ACCOUNT_INSUFFICIENT_STAKE_FUNDS);
        let transfer_amount = amount.into();
        assert!(
            sender_balance.amount() >= transfer_amount,
            ACCOUNT_INSUFFICIENT_STAKE_FUNDS
        );
        sender.apply_stake_debit(transfer_amount);
        self.save_account(&sender_account_id, &sender);

        let (receiver, receiver_account_id) = self.registered_account(receiver_id);

        // Creating a new vault
        *self.vault_id_sequence += 1;
        let vault = Vault(receiver_account_id, transfer_amount);
        self.vaults.insert(&self.vault_id_sequence, &vault);

        // Calling the receiver
        ext_token_receiver::on_receive_with_vault(
            env::predecessor_account_id(),
            transfer_amount.into(),
            self.vault_id_sequence.into(),
            payload,
            &receiver_id.to_string(),
            NO_DEPOSIT.value(),
            gas_to_receiver,
        )
        .then(ext_self::resolve_vault(
            self.vault_id_sequence.into(),
            env::predecessor_account_id(),
            &env::current_account_id(),
            NO_DEPOSIT.value(),
            resolve_vault_gas.value(),
        ))
    }

    fn withdraw_from_vault(
        &mut self,
        vault_id: VaultId,
        receiver_id: ValidAccountId,
        amount: YoctoStake,
    ) {
        let vault_id = vault_id.into();
        let mut vault = self.vaults.get(&vault_id).expect(VAULT_DOES_NOT_EXIST);

        let (_vault_owner, vault_owner_id) =
            self.registered_account(&env::predecessor_account_id());
        if vault_owner_id != vault.owner_id_hash() {
            panic!(VAULT_ACCESS_DENIED);
        }

        let (mut receiver_account, receiver_account_id) =
            self.registered_account(receiver_id.as_ref());

        let transfer_amount = amount.into();
        assert!(vault.balance() >= transfer_amount, VAULT_INSUFFICIENT_FUNDS);
        vault.debit(transfer_amount);
        self.vaults.insert(&vault_id, &vault);

        receiver_account.apply_stake_credit(transfer_amount);
        self.save_account(&receiver_account_id, &receiver_account);
    }

    fn get_total_supply(&self) -> YoctoStake {
        self.total_stake.amount().into()
    }

    fn get_balance(&self, account_id: ValidAccountId) -> YoctoStake {
        let (account, _) = self.registered_account(account_id.as_ref());
        account
            .stake
            .map_or(0.into(), |balance| balance.amount().into())
    }
}

#[near_bindgen]
impl ResolveVaultCallback for StakeTokenContract {
    fn resolve_vault(&mut self, vault_id: VaultId, sender_id: AccountId) -> YoctoStake {
        assert_predecessor_is_self();

        let vault = self
            .vaults
            .remove(&vault_id.into())
            .expect(VAULT_DOES_NOT_EXIST);
        if vault.balance().value() > 0 {
            let (mut account, account_hash_id) = self.registered_account(&sender_id);
            account.apply_stake_credit(vault.balance());
            self.save_account(&account_hash_id, &account);
        }
        vault.balance().into()
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        contract.transfer(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (10 * YOCTO).into(),
        );

        assert_eq!(
            contract.get_balance(ValidAccountId::try_from(sender_account_id).unwrap()),
            (90 * YOCTO).into()
        );
        assert_eq!(
            contract.get_balance(ValidAccountId::try_from(receiver_account_id).unwrap()),
            (10 * YOCTO).into()
        );
        assert_eq!(contract.get_total_supply(), (100 * YOCTO).into());
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        contract.transfer(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (110 * YOCTO).into(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        contract.transfer(ValidAccountId::try_from(account_id).unwrap(), YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "account is not registered: joe.near")]
    fn transfer_sender_not_registered() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = "joe.near".to_string();
        testing_env!(context.clone());
        contract.transfer(ValidAccountId::try_from(account_id).unwrap(), YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "account is not registered: joe.near")]
    fn transfer_receiver_not_registered() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.attached_deposit = 100 * YOCTO;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        contract.transfer(ValidAccountId::try_from("joe.near").unwrap(), YOCTO.into());
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = YoctoStake::from(10 * YOCTO);
        let payload = "payload";

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.clone(),
            payload.to_string(),
        );
        let vault = contract.vaults.get(&contract.vault_id_sequence).unwrap();
        assert_eq!(vault.balance(), transfer_amount.into());
        assert_eq!(vault.owner_id_hash(), Hash::from(receiver_account_id));
        assert_eq!(
            contract.get_balance(ValidAccountId::try_from(sender_account_id).unwrap()),
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
                    assert_eq!(args.payload, payload);
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (110 * YOCTO).into(),
            "paload".to_string(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        contract.transfer_with_vault(
            ValidAccountId::try_from(account_id).unwrap(),
            YOCTO.into(),
            "paload".to_string(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = "joe.near".to_string();
        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(account_id).unwrap(),
            YOCTO.into(),
            "paload".to_string(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        contract.transfer_with_vault(
            ValidAccountId::try_from("joe.near").unwrap(),
            YOCTO.into(),
            "paload".to_string(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = YoctoStake::from(10 * YOCTO);
        let payload = "payload";

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.clone(),
            payload.to_string(),
        );

        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.clone(),
        );

        let vault = contract.vaults.get(&contract.vault_id_sequence).unwrap();
        assert_eq!(vault.balance().value(), 0);
        assert_eq!(
            contract
                .get_balance(ValidAccountId::try_from(receiver_account_id).unwrap())
                .value(),
            transfer_amount.value()
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = YoctoStake::from(10 * YOCTO);
        let payload = "payload";

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
            transfer_amount.clone(),
            payload.to_string(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = YoctoStake::from(10 * YOCTO);
        let payload = "payload";

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.clone(),
            payload.to_string(),
        );

        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from("unknown.near").unwrap(),
            transfer_amount.clone(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = YoctoStake::from(10 * YOCTO);
        let payload = "payload";

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.clone(),
            payload.to_string(),
        );

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.clone(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = YoctoStake::from(10 * YOCTO);
        let payload = "payload";

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.clone(),
            payload.to_string(),
        );

        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (transfer_amount.value() + 1).into(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = YoctoStake::from(10 * YOCTO);
        let payload = "payload";

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.clone(),
            payload.to_string(),
        );

        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            10.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            (transfer_amount.value() + 1).into(),
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
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.register_account();

        context.predecessor_account_id = sender_account_id.to_string();
        testing_env!(context.clone());
        contract.register_account();

        let (mut account, account_id_hash) = contract.registered_account(sender_account_id);
        account.apply_stake_credit((100 * YOCTO).into());
        contract.save_account(&account_id_hash, &account);
        contract.total_stake.credit(account.stake.unwrap().amount());

        let transfer_amount = YoctoStake::from(10 * YOCTO);
        let payload = "payload";

        testing_env!(context.clone());
        contract.transfer_with_vault(
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            transfer_amount.clone(),
            payload.to_string(),
        );

        let withdrawal_amount = YoctoStake::from(4 * YOCTO);
        context.predecessor_account_id = receiver_account_id.to_string();
        testing_env!(context.clone());
        contract.withdraw_from_vault(
            contract.vault_id_sequence.into(),
            ValidAccountId::try_from(receiver_account_id).unwrap(),
            withdrawal_amount.clone(),
        );

        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());
        contract.resolve_vault(
            contract.vault_id_sequence.into(),
            sender_account_id.to_string(),
        );

        assert!(contract.vaults.get(&contract.vault_id_sequence).is_none());
        assert_eq!(
            contract.get_balance(ValidAccountId::try_from(sender_account_id).unwrap()),
            (96 * YOCTO).into()
        );
        assert_eq!(
            contract.get_balance(ValidAccountId::try_from(receiver_account_id).unwrap()),
            withdrawal_amount
        );
    }

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct TransferWithVaultArgs {
        vault_id: U128,
        payload: String,
    }

    #[derive(Deserialize)]
    #[serde(crate = "near_sdk::serde")]
    struct ResolveVaultArgs {
        vault_id: U128,
        sender_id: String,
    }
}
