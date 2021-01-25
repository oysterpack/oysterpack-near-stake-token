use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    json_types::{ValidAccountId, U128},
    serde::{Deserialize, Serialize},
    Promise, PromiseOrValue,
};
use std::{
    cmp::Ordering,
    fmt::{self, Display, Formatter},
    ops::Deref,
};

/// Defines the standard interface for the core Fungible Token contract
/// - [NEP-141](https://github.com/near/NEPs/issues/141)
/// - [NEP-141 Standard discussion](https://github.com/near/NEPs/discussions/146)
///
/// The core standard supports the following features:
/// - [simple token transfers](FungibleToken::ft_transfer)
/// - [token transfers between contracts](FungibleToken::ft_transfer_call)
/// - accounting for [total token supply](FungibleToken::ft_total_supply) and
///   [account balances](FungibleToken::ft_balance_of)
///
/// ## Notes
/// - it doesn't include token metadata standard that will be covered by a separate NEP, because the
///   metadata may evolve.
/// - it also doesn't include account registration standard that also must be covered by a separate
///   NEP because it can be reused for other contract.
///
/// ### Security
/// Requirement for accept attached deposits (#\[payable\])
/// Due to the nature of function-call permission access keys on NEAR protocol, the method that
/// requires an attached deposit can't be called by the restricted access key. If the token contract
/// requires an attached deposit of at least 1 yoctoNEAR on transfer methods, then the function-call
/// restricted access key will not be able to call them without going through the wallet confirmation.
/// This prevents some attacks like fishing through an authorization to a token contract.
///
/// This 1 yoctoNEAR is enforced by this standard.
///
/// ### Transfer Call Refunds
/// If the receiver contract is malicious or incorrectly implemented, then the receiver's promise
/// result may be invalid and the required balance may not be available on the receiver's account.
/// In this case the refund can't be provided provided to the sender. This is prevented by #122
/// standard that locks funds into a temporary vault and prevents receiver from overspending the
/// funds and later retuning invalid value. But if this flaw exist in this standard, it's not an
/// issue for the sender account. It only affects the transfer amount and the receiver's account
/// balance. The receiver can't overspend tokens from the sender outside of sent amount, so this
/// standard must be considered as safe as #122
///
pub trait FungibleToken {
    /// Enables simple transfer between accounts.
    ///
    /// - Transfers positive `amount` of tokens from the `env::predecessor_account_id` to `receiver_id`.
    /// - Both accounts must be registered with the contract for transfer to succeed.
    /// - Sender account is required to attach exactly 1 yoctoNEAR to the function call - see security
    ///   section of the standard.
    ///   - the yoctoNEAR will be credited to the sender account's NEAR balance
    ///
    /// Arguments:
    /// - `receiver_id` - the account ID of the receiver.
    /// - `amount` - the amount of tokens to transfer. must be a positive number in decimal string representation.
    /// - `memo` - an optional string field in a free form to associate a memo with this transfer.
    ///
    /// ## Panics
    /// - if the attached deposit does not equal 1 yoctoNEAR
    /// - if either sender or receiver accounts are not registered
    /// - if amount is zero
    /// - if the sender account has insufficient funds to fulfill the request
    ///
    /// GAS REQUIREMENTS: 10 TGas
    /// #\[payable\]
    fn ft_transfer(&mut self, receiver_id: ValidAccountId, amount: TokenAmount, memo: Option<Memo>);

    /// Transfer to a contract with a callback.
    ///
    /// Transfers positive `amount` of tokens from the `env::predecessor_account_id` to `receiver_id`
    /// account. Then calls [`OnTransfer::ft_on_transfer`] method on `receiver_id` contract
    /// and attaches a callback to resolve this transfer.
    ///
    /// [`OnTransfer::ft_on_transfer`] method  must return the amount of tokens unused by
    /// the receiver contract, the remaining tokens must be refunded to the `predecessor_account_id`
    /// by the resolve transfer callback.
    ///
    /// Token contract must pass all the remaining unused gas to [`OnTransfer::ft_on_transfer`]
    ///
    /// Malicious or invalid behavior by the receiver's contract:
    /// - If the receiver contract promise fails or returns invalid value, the full transfer amount
    ///   must be refunded.
    /// - If the receiver contract overspent the tokens, and the `receiver_id` balance is lower
    ///   than the required refund amount, the remaining balance must be refunded.
    ///
    /// Both accounts must be registered with the contract for transfer to succeed.
    /// Sender must attach exactly 1 yoctoNEAR - see security section of the standard.
    ///
    /// Arguments:
    /// - `receiver_id` - the account ID of the receiver contract. This contract will be called.
    /// - `amount` - the amount of tokens to transfer. Must be a positive number in decimal string representation.
    /// - `msg` - a string message that will be passed to `ft_on_transfer` contract call.
    /// - `memo` - an optional string field in a free form to associate a memo with this transfer.
    ///
    /// Returns a promise to resolve transfer call which will return the used amount - [`ResolveTransferCall`]
    ///
    /// ## Panics
    /// - if the attached deposit is not exactly 1 yoctoNEAR
    /// - if either sender or receiver accounts are not registered
    /// - if amount is zero
    /// - if the sender account has insufficient funds to fulfill the transfer request
    ///
    /// GAS REQUIREMENTS: 40 TGas + gas for receiver call
    /// #\[payable\]
    fn ft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
        msg: TransferCallMessage,
        memo: Option<Memo>,
    ) -> Promise;

    fn ft_total_supply(&self) -> TokenAmount;

    /// If the account doesn't exist, then zero is returned.
    fn ft_balance_of(&self, account_id: ValidAccountId) -> TokenAmount;
}

/// Receiver of the Fungible Token for [`FungibleToken::ft_transfer_call`] calls.
pub trait TransferReceiver {
    /// Callback to receive tokens.
    ///
    /// Called by fungible token contract `env::predecessor_account_id` after `transfer_call` was initiated by
    /// `sender_id` of the given `amount` with the transfer message given in `msg` field.
    /// The `amount` of tokens were already transferred to this contract account and ready to be used.
    ///
    /// The method must return the amount of tokens that are not used/accepted by this contract from
    /// the transferred amount, e.g.:
    /// - The transferred amount was `500`, the contract completely takes it and must return `0`.
    /// - The transferred amount was `500`, but this transfer call only needs `450` for the action passed in the `msg`
    ///   field, then the method must return `50`.
    /// - The transferred amount was `500`, but the action in `msg` field has expired and the transfer must be
    ///   cancelled. The method must return `500` or panic.
    ///
    /// Arguments:
    /// - `sender_id` - the account ID that initiated the transfer.
    /// - `amount` - the amount of tokens that were transferred to this account.
    /// - `msg` - a string message that was passed with this transfer call.
    ///
    /// Returns the amount of tokens that are used/accepted by this contract from the transferred amount.
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: TokenAmount,
        msg: TransferCallMessage,
    ) -> PromiseOrValue<TokenAmount>;
}

/// Callback on fungible token contract to resolve transfer.
pub trait ResolveTransferCall {
    /// Callback to resolve transfer.
    /// Private method (`env::predecessor_account_id == env::current_account_id`).
    ///
    /// Called after the receiver handles the transfer call and returns unused token amount.
    ///
    /// This method must get `unused_amount` from the receiver's promise result and refund the
    /// `unused_amount` from the receiver's account back to the `sender_id` account.
    ///
    /// Arguments:
    /// - `sender_id` - the account ID that initiated the transfer.
    /// - `receiver_id` - the account ID of the receiver contract.
    /// - `amount` - the amount of tokens that were transferred to receiver's account.
    ///
    /// Promise result data dependency (`unused_amount`):
    /// - the amount of tokens that were unused by receiver's contract.
    /// - Received from `on_ft_receive`
    /// - `unused_amount` must be `U128` in range from `0` to `amount`. All other invalid values
    ///   are considered to be equal to be the total transfer amount.
    ///
    /// Returns amount that was refunded back to the sender.
    ///
    /// #\[private\]
    fn ft_resolve_transfer_call(
        &mut self,
        sender_id: ValidAccountId,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
        // NOTE: #[callback_result] is not supported yet and has to be handled using lower level interface.
        //
        // #[callback_result]
        // unused_amount: CallbackResult<TokenAmount>,
    ) -> PromiseOrValue<TokenAmount>;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenAmount(pub U128);

impl From<u128> for TokenAmount {
    fn from(value: u128) -> Self {
        Self(value.into())
    }
}

impl TokenAmount {
    pub fn value(&self) -> u128 {
        self.0 .0
    }
}

impl Display for TokenAmount {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0 .0.fmt(f)
    }
}

impl PartialOrd for TokenAmount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value().partial_cmp(&other.value())
    }
}

impl Default for TokenAmount {
    fn default() -> Self {
        Self(U128(0))
    }
}

/// > Similarly to bank transfer and payment orders, the memo argument allows to reference transfer
/// > to other event (on-chain or off-chain). It is a schema less, so user can use it to reference
/// > an external document, invoice, order ID, ticket ID, or other on-chain transaction. With memo
/// > you can set a transfer reason, often required for compliance.
/// >
/// > This is also useful and very convenient for implementing FATA (Financial Action Task Force)
/// > guidelines (section 7(b) ). Especially a requirement for VASPs (Virtual Asset Service Providers)
/// > to collect and transfer customer information during transactions. VASP is any entity which
/// > provides to a user token custody, management, exchange or investment services.
/// > With ERC-20 (and NEP-21) it is not possible to do it in atomic way. With memo field, we can
/// > provide such reference in the same transaction and atomically bind it to the money transfer.
///
/// - https://github.com/near/NEPs/issues/136
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct Memo(pub String);

impl Deref for Memo {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for Memo {
    fn from(memo: &str) -> Self {
        Self(memo.to_string())
    }
}

impl Display for Memo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// > The mint, send and burn processes can all make use of a data and operatorData fields which are
/// > passed to any movement (mint, send or burn). Those fields may be empty for simple use cases,
/// > or they may contain valuable information related to the movement of tokens, similar to
/// > information attached to a bank transfer by the sender or the bank itself.
/// > The use of a data field is equally present in other standard proposals such as EIP-223, and
/// > was requested by multiple members of the community who reviewed this standard.
/// >
/// - https://eips.ethereum.org/EIPS/eip-777#data
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct TransferCallMessage(pub String);

impl Deref for TransferCallMessage {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for TransferCallMessage {
    fn from(memo: &str) -> Self {
        Self(memo.to_string())
    }
}

impl Display for TransferCallMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
