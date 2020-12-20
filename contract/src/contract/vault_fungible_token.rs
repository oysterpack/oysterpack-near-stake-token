#![allow(unused_imports)]

use crate::domain::Vault;
use crate::errors::vault_fungible_token::{
    ACCOUNT_INSUFFICIENT_STAKE_FUNDS, RECEIVER_MUST_NOT_BE_SENDER,
};
use crate::{
    core::Hash,
    domain::{self, Account, RedeemLock, RedeemStakeBatch, StakeBatch},
    interface::{
        ext_self, ext_token_receiver, BatchId, RedeemStakeBatchReceipt, ResolveVaultCallback,
        StakeTokenValue, StakingService, VaultFungibleToken, VaultId, YoctoNear, YoctoStake,
    },
    near::NO_DEPOSIT,
    StakeTokenContract,
};
use near_sdk::json_types::ValidAccountId;
use near_sdk::{
    env, ext_contract,
    json_types::U128,
    near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId, Promise,
};

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
        unimplemented!()
    }

    fn get_total_supply(&self) -> YoctoStake {
        unimplemented!()
    }

    fn get_balance(&self, account_id: ValidAccountId) -> YoctoStake {
        unimplemented!()
    }
}

#[near_bindgen]
impl ResolveVaultCallback for StakeTokenContract {
    fn resolve_vault(&mut self, vault_id: VaultId, sender_id: AccountId) -> YoctoStake {
        unimplemented!()
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
