use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::{
    ext_contract,
    serde::{Deserialize, Serialize},
    AccountId, Promise, PromiseOrValue,
};

/// The design intent is to decouple the token asset from the token transfer protocol.
///
/// - Fungible token supports 1 or more [TransferProtocol]s as specified per [MetaData]
/// - Accounts must register with the token contract and pay for account storage fees.
///   - account storage fees are escrowed and refunded when the account unregisters
///   - account chooses the transfer protocol to use as transfer recipient
/// - FT has generic [transfer] function interface
/// - sender account does not choose the transfer protocol - the receiver account chooses how they
///   want to receive the tokens
///
/// The key advantage of this design is that it decouples the protocol interface from the implementation.
/// The problem with all of the "standard" interfaces is that they are too tightly coupled with implementation.
/// We need decoupled interface that will allow transfer protocols to evolve.
pub trait FungibleToken: AccountRegistry {
    fn metadata() -> Metadata;

    /// Returns total supply.
    /// MUST equal to total_amount_of_token_minted - total_amount_of_token_burned
    fn total_supply(&self) -> U128;

    /// Returns the token balance for `holder` account
    fn balance_of(&self, account_id: ValidAccountId) -> U128;

    /// ## Panics
    /// - if accounts are not registered
    /// - insufficient funds
    fn transfer(
        &mut self,
        receiver_id: ValidAccountId,
        amount: U128,
        msg: Option<String>,
        memo: Option<String>,
    ) -> PromiseOrValue<TransferProtocol>;
}

/// Suggested protocol names:
/// - NEP_122
/// - NEP_136
/// - NEP_21
///
/// - each protocol defines min amount of gas required excluding gas required to cover `msg` `memo`
pub struct TransferProtocol(String, Gas);

pub struct Gas(pub u64);

pub trait AccountRegistry {
    /// Registers the predecessor account ID with the contract.
    /// The account is required to pay for its storage. Storage fees will be escrowed and refunded
    /// when the account is unregistered.
    ///
    /// #[payable]
    /// - storage escrow fee is required
    ///   - use [account_storage_escrow_fee] to lookup the required storage fee amount
    /// - any amount above the storage fee will be refunded
    ///
    /// ## Panics
    /// - if deposit is not enough to cover storage fees
    /// - is account is already registered
    /// - if transfer protocol is not supported
    ///
    /// #[payable]
    fn register_account(&mut self, transfer_protocol: TransferProtocol);

    /// Unregisters the account and refunds the escrowed storage fees.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if registered account has funds
    fn unregister_account(&mut self);

    /// changes the account's transfer type
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if transfer protocol is not supported
    fn set_transfer_type(&mut self, transfer_protocol: TransferProtocol);

    /// Burns owned token funds unregisters the account. Escrowed storage fees are refunded.
    ///
    /// ## Panics
    /// - if account is not registered
    fn burn_account(&mut self);

    ////////////////////////////
    ///     VIEW METHODS    ///
    //////////////////////////

    /// Returns the required deposit amount that is required for account registration.
    fn account_storage_fee(&self) -> U128;

    fn account_registered(&self, account_id: ValidAccountId) -> bool;

    /// returns None if the account is not registered
    fn account_transfer_protocol(&self, account_id: ValidAccountId) -> Option<String>;

    fn total_registered_accounts(&self) -> U128;
}

/// Each token must have 18 digits precision (decimals)
pub const DECIMALS: u8 = 18;

pub struct Metadata {
    pub name: String,
    pub symbol: String,

    /// URL to additional resources about the token.
    pub reference: String,

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
    pub granularity: U128,

    /// Transfer protocols that are supported by the token contract
    pub supported_transfer_protocols: Vec<TransferProtocol>,
}

///////////////////////////////////////////////////////////////////////////////////////////////////
///
/// Example Transfer Protocol Specs
///
///////////////////////////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
pub trait TransferCall {
    /// Transfer `amount` of tokens from the predecessor account to a `recipient` contract.
    /// `recipient` MUST be a smart contract address.
    /// The recipient contract MUST implement [TransferCallRecipient] interface.
    /// `msg`: is a message sent to the recipient. It might be used to send additional call
    //      instructions.
    /// `memo`: arbitrary data with no specified format used to link the transaction with an
    ///     external event. If referencing a binary data, it should use base64 serialization.
    /// The function panics if the predecessor doesn't have sufficient amount of shares.
    fn transfer_call(&mut self, recipient: ValidAccountId, amount: U128, msg: String, memo: String);
}

/// Interface for recipient call on fungible-token transfers.
/// `token` is an account address of the token  - a smart-contract defining the token
///     being transferred.
/// `from` is an address of a previous holder of the tokens being sent
#[ext_contract]
pub trait TransferCallRecipient {
    fn on_ft_receive(&mut self, from: ValidAccountId, amount: U128, msg: String);
}
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
pub trait VaultBasedTransfer {
    /// Transfer to a contract with payload
    /// Gas requirement: 40+ TGas or 40000000000000 Gas.
    /// Consumes: 30 TGas and the remaining gas is passed to the `receiver_id` (at least 10 TGas)
    /// Should be called by the balance owner.
    /// Returns a promise, that will result in the unspent balance from the transfer `amount`.
    ///
    /// Actions:
    /// - Withdraws `amount` from the `predecessor_id` account.
    /// - Creates a new local vault with a new unique `safe_id` with the following content:
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
        amount: U128,
        msg: String,
        memo: String,
    ) -> Promise;

    /// Used by token receiver to Withdraw from a given safe.
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
    fn withdraw_from_vault(&mut self, vault_id: VaultId, receiver_id: ValidAccountId, amount: U128);
}

/// Must be implemented by contracts that support [VaultBasedTransfer] token transfers
#[ext_contract]
pub trait ExtTokenVaultReceiver {
    fn on_receive_with_vault(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        vault_id: VaultId,
        payload: String,
    );
}

#[ext_contract]
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
    ///
    /// #[private]
    fn resolve_vault(&mut self, vault_id: VaultId, sender_id: AccountId) -> U128;
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct VaultId(pub U128);
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
pub trait DirectTransfer {
    /// Transfer `amount` of tokens from the predecessor account to a `recipient` account.
    /// If recipient is a smart-contract, then `transfer_call` should be used instead.
    /// `recipient` MUST NOT be a smart-contract.
    /// `msg`: is a message for recipient. It might be used to send additional call
    //      instructions.
    /// `memo`: arbitrary data with no specified format used to link the transaction with an
    ///     external data. If referencing a binary data, it should use base64 serialization.
    /// The function panics if the token doesn't refer to any registered pool or the predecessor
    /// doesn't have sufficient amount of shares.
    fn transfer(
        &mut self,
        recipient: ValidAccountId,
        amount: U128,
        msg: String,
        memo: String,
    ) -> bool;
}
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
