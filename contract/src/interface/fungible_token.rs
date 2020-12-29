use crate::domain::{self, Gas};
use near_sdk::json_types::{ValidAccountId, U128};
#[allow(unused_imports)]
use near_sdk::AccountId;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    ext_contract,
    serde::{Deserialize, Serialize},
    Promise,
};

/// - Fungible token supports 1 or more [TransferProtocol]s as specified per [MetaData]
/// - Accounts must register with the token contract and pay for account storage fees.
///   - account storage fees are escrowed and refunded when the account unregisters
///   - account chooses the transfer protocol to use as transfer recipient
pub trait FungibleToken {
    fn metadata(&self) -> Metadata;

    /// Returns total supply.
    /// MUST equal to total_amount_of_token_minted - total_amount_of_token_burned
    fn total_supply(&self) -> U128;

    /// Returns the token balance for `holder` account
    fn balance(&self, account_id: ValidAccountId) -> U128;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Metadata {
    pub name: String,
    pub symbol: String,

    /// URL to additional resources about the token.
    pub reference: Option<String>,

    /// the smallest part of the token that’s (denominated in e18) not divisible
    /// In other words, the granularity is the smallest amount of tokens (in the internal denomination)
    /// which MAY be minted, sent or burned at any time.
    /// - The following rules MUST be applied regarding the granularity:
    /// - The granularity value MUST be set at creation time.
    /// - The granularity value MUST NOT be changed, ever.
    /// - The granularity value MUST be greater than or equal to 1.
    /// - All balances MUST be a multiple of the granularity.
    /// - Any amount of tokens (in the internal denomination) minted, sent or burned MUST be a
    ///   multiple of the granularity value.
    /// - Any operation that would result in a balance that’s not a multiple of the granularity value
    ///   MUST be considered invalid, and the transaction MUST revert.
    ///
    /// NOTE: Most tokens SHOULD be fully partition-able. I.e., this function SHOULD return 1 unless
    ///       there is a good reason for not allowing any fraction of the token.
    pub granularity: u8,

    /// Transfer protocols that are supported by the token contract
    pub supported_transfer_protocols: Vec<TransferProtocol>,
}

impl Metadata {
    /// Each token must have 18 digits precision (decimals)
    pub const DECIMALS: u8 = 18;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TransferProtocol {
    /// Suggested protocol names:
    /// - simple - NEP-21
    /// - allowance - NEP 21
    /// - vault_transfer - NEP-122
    /// - transfer_and_notify - NEP-136
    pub name: String,
    /// - each protocol defines min amount of gas required, excluding gas required to cover `msg` `memo`
    pub gas: Gas,
}

impl TransferProtocol {
    pub fn simple(gas: Gas) -> Self {
        Self {
            name: "simple".to_string(),
            gas,
        }
    }

    pub fn allowance(gas: Gas) -> Self {
        Self {
            name: "allowance".to_string(),
            gas,
        }
    }

    pub fn vault_transfer(gas: Gas) -> Self {
        Self {
            name: "vault_transfer".to_string(),
            gas,
        }
    }

    pub fn transfer_and_notify(gas: Gas) -> Self {
        Self {
            name: "transfer_and_notify".to_string(),
            gas,
        }
    }
}

pub trait SimpleTransfer {
    /// Simple direct transfers between registered accounts.
    ///
    /// Gas requirement: 5 TGas
    /// Should be called by the balance owner.
    /// Requires that the sender and the receiver accounts be registered.
    ///
    /// Actions:
    /// - Transfers `amount` of tokens from `predecessor_id` to `recipient`.
    ///
    /// ## Panics
    /// - if predecessor account is not registered - sender account
    /// - if [recipient] account is not registered
    /// - if sender account is same as receiver account
    /// - if account balance has insufficient funds for transfer
    fn transfer(
        &mut self,
        recipient: ValidAccountId,
        amount: U128,
        msg: Option<String>,
        memo: Option<String>,
    );
}

pub trait TransferCall {
    /// Transfer `amount` of tokens from the predecessor account to a `recipient` contract.
    /// The recipient contract MUST implement [TransferCallRecipient] interface. The tokens are
    /// deposited but locked in the recipient account until the transfer has been confirmed by the
    /// recipient contract and then finalized. The transfer workflow steps are:
    /// 1. sender initiates the transfer via `transder_call`
    /// 2. token transfers the funds from the sender's account to the recipient's account but locks
    ///    the transfer amount on the recipient account. The locked tokens cannot be used until
    ///    the recipient contract confirms the transfer.
    /// 3. The recipient contract is then notified of the transfer via [`TransferCallRecipient::on_ft_receive`].
    /// 4. Once the transfer notification call completes, then the [`TransferCallRecipient::on_ft_receive`]
    ///    callback on the token contract is invoked to finalize the transfer. If the recipient contract
    ///    successfully completed the transfer notification call, then the funds are unlocked
    ///    via the [`FinalizeTransferCallback::finalize_ft_transfer`] callback. If the [`TransferCallRecipient::on_ft_receive`]
    ///    call fails for any reason, then the fund transfer is rolled back in the finalize callback.
    ///
    /// `msg`: is a message sent to the recipient. It might be used to send additional call
    //      instructions.
    /// `memo`: arbitrary data with no specified format used to link the transaction with an
    ///     external event. If referencing a binary data, it should use base64 serialization.
    ///
    /// ## Panics
    /// - if accounts are not registered
    /// - insufficient funds
    fn transfer_call(
        &mut self,
        recipient: ValidAccountId,
        amount: U128,
        msg: Option<String>,
        memo: Option<String>,
    ) -> Promise;
}

pub trait FinalizeTransferCallback {
    /// Finalizes the token transfer
    ///
    /// Actions:
    /// - if the call `TransferCallRecipient::on_ft_receive` succeeds, then commit the transfer,
    ///   i.e., unlock the balance on the recipient account
    /// - else rollback the transfer by returning the locked balance to the sender
    ///
    /// #[private]
    fn finalize_ft_transfer(&mut self, sender: AccountId, recipient: AccountId, amount: U128);
}

/// Interface for recipient call on fungible-token transfers.
/// `token` is an account address of the token  - a smart-contract defining the token
///     being transferred.
/// `from` is an address of a previous holder of the tokens being sent
#[ext_contract(ext_transfer_call_recipient)]
pub trait TransferCallRecipient {
    fn on_ft_receive(
        &mut self,
        from: ValidAccountId,
        amount: U128,
        msg: Option<String>,
        memo: Option<String>,
    );
}

#[ext_contract(ext_self_finalize_transfer_callback)]
pub trait ExtFinalizeTransferCallback {
    /// Finalizes the token transfer
    ///
    /// Actions:
    /// - if the call `TransferCallRecipient::on_ft_receive` succeeds, then commit the transfer,
    ///   i.e., unlock the balance on the recipient account
    /// - else rollback the transfer by returning the locked balance to the sender
    ///
    /// #[private]
    fn finalize_ft_transfer(&mut self, sender: AccountId, recipient: AccountId, amount: U128);
}

/// Implements [NEP-122 vault based fungible token standard](https://github.com/near/NEPs/issues/122)
/// with the following modifications:
/// - all token owners must be registered with the contract, which implies that token transfers can
///   only be between registered accounts
///   - this removes the need to require an attached deposit on each transfer because the accounts
///     are pre-registered
///   - eliminates transfers to non-existent accounts
/// - `transfer_raw` has been moved to [`SimpleTransfer::transfer`]
/// - `payload` has been replaced with `msg` and `memo` optional args
pub trait VaultBasedTransfer {
    /// Transfer to a contract with payload
    /// Gas requirement: 40+ TGas or 40000000000000 Gas.
    /// Consumes: 30 TGas and the remaining gas is passed to the `recipient` (at least 10 TGas)
    /// Should be called by the balance owner.
    /// Returns a promise, that will result in the unspent balance from the transfer `amount`.
    ///
    /// Actions:
    /// - Withdraws `amount` from the `predecessor_id` account.
    /// - Creates a new local safe with a new unique `safe_id` with the following content:
    ///     `{sender_id: predecessor_id, amount: amount, recipient: recipient}`
    /// - Saves this safe to the storage.
    /// - Calls on `recipient` method `on_token_receive(sender_id: predecessor_id, amount, safe_id, payload)`/
    /// - Attaches a self callback to this promise `resolve_safe(safe_id, sender_id)`
    ///
    /// ## Panics
    /// - if predecessor account is not registered
    /// - if [recipient] account is not registered
    /// - if sender account is same as receiver account
    /// - if account balance has insufficient funds for transfer
    fn transfer_with_vault(
        &mut self,
        recipient: ValidAccountId,
        amount: U128,
        msg: Option<String>,
        memo: Option<String>,
    ) -> Promise;

    /// Withdraws from a given vault and transfers the funds to the specified receiver account ID.
    ///
    /// Gas requirement: 5 TGas
    /// Should be called by the contract that owns a given safe.
    ///
    /// Actions:
    /// - checks that the safe with `vault_id` exists and `predecessor_id == vault.recipient`
    /// - withdraws `amount` from the vault or panics if `vault.amount < amount`
    /// - deposits `amount` on the `recipient`
    ///
    /// ## panics
    /// - if predecessor account is not registered
    /// - if predecessor account does not own the vault
    /// - if [recipient] account is not registered
    /// - if vault balance has insufficient funds for transfer
    fn withdraw_from_vault(&mut self, vault_id: VaultId, recipient: ValidAccountId, amount: U128);
}

/// implements required callbacks defined in [ExtResolveVaultCallback]
pub trait ResolveVaultCallback {
    /// Resolves a given vault, i.e., transfers any remaining vault balance to the sender account
    /// and then deletes the vault. Returns the vault remaining balance.
    ///
    /// Gas requirement: 5 TGas
    ///
    /// Actions:
    /// - Reads safe with `safe_id`
    /// - Deposits remaining `safe.amount` to `sender_id`
    /// - Deletes the safe
    /// - Returns the total withdrawn amount from the safe `original_amount - safe.amount`.
    /// #\[private\]
    ///
    /// ## Panics
    /// - if not called by self as callback
    /// - following panics should never happen (if they do, then there is a bug in the code)
    ///   - if the sender account is not registered
    ///   - if the vault does not exist
    fn resolve_vault(&mut self, vault_id: VaultId, sender_id: AccountId) -> U128;
}

/// Must be implemented by contracts that support [VaultBasedTransfer] token transfers
#[ext_contract(ext_token_receiver)]
pub trait ExtTokenVaultReceiver {
    /// Called when a given amount of tokens is locked in a safe by a given sender with payload.
    /// Gas requirements: 2+ BASE
    /// Should be called by the fungible token contract
    ///
    /// This methods should withdraw tokens from the safe and act on them. When this method returns a value, the
    /// safe will be released and the unused tokens from the safe will be returned to the sender.
    /// There are bunch of options what the contract can do. E.g.
    /// - Option 1: withdraw and account internally
    ///     - Increase inner balance by `amount` for the `sender_id` of a token contract ID `predecessor_id`.
    ///     - Promise call `withdraw_from_safe(safe_id, recipient: env::current_account_id(), amount)` to withdraw the amount to this contract
    ///     - Return the promise
    /// - Option 2: Simple redirect to another account
    ///     - Promise call `withdraw_from_safe(safe_id, recipient: ANOTHER_ACCOUNT_ID, amount)` to withdraw to `ANOTHER_ACCOUNT_ID`
    ///     - Return the promise
    /// - Option 3: Partial redirect to another account (e.g. with commission)
    ///     - Promise call `withdraw_from_safe(safe_id, recipient: ANOTHER_ACCOUNT_ID, amount: ANOTHER_AMOUNT)` to withdraw to `ANOTHER_ACCOUNT_ID`
    ///     - Chain with (using .then) promise call `withdraw_from_safe(safe_id, recipient: env::current_account_id(), amount: amount - ANOTHER_AMOUNT)` to withdraw to self
    ///     - Return the 2nd promise
    /// - Option 4: redirect some of the payments and call another contract `NEW_RECEIVER_ID`
    ///     - Promise call `withdraw_from_safe(safe_id, recipient: current_account_id, amount)` to withdraw the amount to this contract
    ///     - Chain with promise call `transfer_with_safe(recipient: recipient, amount: SOME_AMOUNT, payload: NEW_PAYLOAD)`
    ///     - Chain with the promise call to this contract to handle callback (in case we want to refund).
    ///     - Return the callback promise.
    fn on_receive_with_vault(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        vault_id: VaultId,
        msg: Option<String>,
        memo: Option<String>,
    );
}

#[ext_contract(ext_self_resolve_vault_callback)]
pub trait ExtResolveVaultCallback {
    /// Resolves a given vault - transfers vault remoining balance back to sender account and deletes
    /// the vault.
    ///
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
    fn resolve_vault(&mut self, vault_id: VaultId, sender_id: AccountId) -> U128;
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
