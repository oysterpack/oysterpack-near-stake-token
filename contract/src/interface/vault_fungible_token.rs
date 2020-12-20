use crate::{domain, interface::YoctoStake};
use near_sdk::json_types::U128;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    ext_contract,
    json_types::ValidAccountId,
    serde::{Deserialize, Serialize},
    Promise,
};
#[allow(unused_imports)]
use near_vm_logic::types::AccountId;

pub trait VaultFungibleToken {
    /// Simple transfers
    /// Gas requirement: 5 TGas or 5000000000000 Gas
    /// Should be called by the balance owner.
    /// Requires that the sender and the receiver accounts be registered.
    ///
    /// Actions:
    /// - Transfers `amount` of tokens from `predecessor_id` to `receiver_id`.
    ///
    /// ## Panics
    /// - if predecessor account is not registered - sender account
    /// - if [receiver_id] account is not registered
    /// - if sender account is same as receiver account
    /// - if account balance has insufficient funds for transfer
    /// - if there is no attached deposit
    fn transfer(&mut self, receiver_id: ValidAccountId, amount: YoctoStake);

    /// Transfer to a contract with payload
    /// Gas requirement: 40+ TGas or 40000000000000 Gas.
    /// Consumes: 30 TGas and the remaining gas is passed to the `receiver_id` (at least 10 TGas)
    /// Should be called by the balance owner.
    /// Returns a promise, that will result in the unspent balance from the transfer `amount`.
    ///
    /// Actions:
    /// - Withdraws `amount` from the `predecessor_id` account.
    /// - Creates a new local safe with a new unique `safe_id` with the following content:
    ///     `{sender_id: predecessor_id, amount: amount, receiver_id: receiver_id}`
    /// - Saves this safe to the storage.
    /// - Calls on `receiver_id` method `on_token_receive(sender_id: predecessor_id, amount, safe_id, payload)`/
    /// - Attaches a self callback to this promise `resolve_safe(safe_id, sender_id)`
    ///
    /// ## Panics
    /// - if predecessor account is not registered
    /// - if [receiver_id] account is not registered
    /// - if sender account is same as receiver account
    /// - if account balance has insufficient funds for transfer
    /// - if there is no attached deposit
    fn transfer_with_vault(
        &mut self,
        receiver_id: ValidAccountId,
        amount: YoctoStake,
        payload: String,
    ) -> Promise;

    /// Withdraws from a given safe
    /// Gas requirement: 5 TGas or 5000000000000 Gas
    /// Should be called by the contract that owns a given safe.
    ///
    /// Actions:
    /// - checks that the safe with `safe_id` exists and `predecessor_id == safe.receiver_id`
    /// - withdraws `amount` from the safe or panics if `safe.amount < amount`
    /// - deposits `amount` on the `receiver_id`
    ///
    /// ## panics
    /// - if predecessor account is not registered
    /// - if [receiver_id] account is not registered
    /// - if vault balance has insufficient funds for transfer
    fn withdraw_from_vault(
        &mut self,
        vault_id: VaultId,
        receiver_id: ValidAccountId,
        amount: YoctoStake,
    );

    fn get_total_supply(&self) -> YoctoStake;

    fn get_balance(&self, account_id: ValidAccountId) -> YoctoStake;
}

/// implements required callbacks defined in [ExtSelf]
pub trait ResolveVaultCallback {
    /// Resolves a given vault
    /// Gas requirement: 5 TGas or 5000000000000 Gas
    /// A callback. Should be called by this fungible token contract (`current_account_id`)
    /// Returns the remaining balance.
    ///
    /// Actions:
    /// - Reads safe with `safe_id`
    /// - Deposits remaining `safe.amount` to `sender_id`
    /// - Deletes the safe
    /// - Returns the total withdrawn amount from the safe `original_amount - safe.amount`.
    /// #[private]
    fn resolve_vault(&mut self, vault_id: VaultId, sender_id: AccountId) -> YoctoStake;
}

#[ext_contract(ext_token_receiver)]
pub trait ExtTokenReceiver {
    /// Called when a given amount of tokens is locked in a safe by a given sender with payload.
    /// Gas requirements: 2+ BASE
    /// Should be called by the fungible token contract
    ///
    /// This methods should withdraw tokens from the safe and act on them. When this method returns a value, the
    /// safe will be released and the unused tokens from the safe will be returned to the sender.
    /// There are bunch of options what the contract can do. E.g.
    /// - Option 1: withdraw and account internally
    ///     - Increase inner balance by `amount` for the `sender_id` of a token contract ID `predecessor_id`.
    ///     - Promise call `withdraw_from_safe(safe_id, receiver_id: env::current_account_id(), amount)` to withdraw the amount to this contract
    ///     - Return the promise
    /// - Option 2: Simple redirect to another account
    ///     - Promise call `withdraw_from_safe(safe_id, receiver_id: ANOTHER_ACCOUNT_ID, amount)` to withdraw to `ANOTHER_ACCOUNT_ID`
    ///     - Return the promise
    /// - Option 3: Partial redirect to another account (e.g. with commission)
    ///     - Promise call `withdraw_from_safe(safe_id, receiver_id: ANOTHER_ACCOUNT_ID, amount: ANOTHER_AMOUNT)` to withdraw to `ANOTHER_ACCOUNT_ID`
    ///     - Chain with (using .then) promise call `withdraw_from_safe(safe_id, receiver_id: env::current_account_id(), amount: amount - ANOTHER_AMOUNT)` to withdraw to self
    ///     - Return the 2nd promise
    /// - Option 4: redirect some of the payments and call another contract `NEW_RECEIVER_ID`
    ///     - Promise call `withdraw_from_safe(safe_id, receiver_id: current_account_id, amount)` to withdraw the amount to this contract
    ///     - Chain with promise call `transfer_with_safe(receiver_id: NEW_RECEIVER_ID, amount: SOME_AMOUNT, payload: NEW_PAYLOAD)`
    ///     - Chain with the promise call to this contract to handle callback (in case we want to refund).
    ///     - Return the callback promise.
    fn on_receive_with_vault(
        &mut self,
        sender_id: AccountId,
        amount: YoctoStake,
        vault_id: VaultId,
        payload: String,
    );
}

#[ext_contract(ext_self)]
pub trait ExtSelf {
    /// Resolves a given vault
    /// Gas requirement: 5 TGas or 5000000000000 Gas
    /// A callback. Should be called by this fungible token contract (`current_account_id`)
    /// Returns the remaining balance.
    ///
    /// Actions:
    /// - Reads safe with `safe_id`
    /// - Deposits remaining `safe.amount` to `sender_id`
    /// - Deletes the safe
    /// - Returns the total withdrawn amount from the safe `original_amount - safe.amount`.
    /// #[private]
    fn resolve_vault(&mut self, vault_id: VaultId, sender_id: AccountId) -> YoctoStake;
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct VaultId(pub U128);

impl From<u128> for VaultId {
    fn from(value: u128) -> Self {
        Self(value.into())
    }
}

impl From<domain::VaultId> for VaultId {
    fn from(id: domain::VaultId) -> Self {
        Self(id.0.into())
    }
}
