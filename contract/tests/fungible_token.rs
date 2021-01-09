use near_sdk::{
    json_types::{ValidAccountId, U128},
    serde::{Deserialize, Serialize},
    Promise, PromiseOrValue,
};
use std::cmp::Ordering;
use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    ops::{Deref, DerefMut},
};

/// Core functionality of a Fungible Token contract
pub trait FungibleTokenCore {
    /// Transfer to a contract with a callback.
    /// Transfers positive `amount` of tokens from the `env::predecessor_account_id` to `receiver_id` account. Then
    /// calls `on_ft_receive` method on `receiver_id` contract and attaches a callback to resolve this transfer.
    /// `on_ft_receive` method should return the amount of tokens used by the receiver contract, the remaining tokens
    /// should be refunded to the `predecessor_account_id` at the resolve transfer callback.
    ///
    /// Token contract should pass all the remaining unused gas to the `on_ft_receive` call.
    ///
    /// Malicious or invalid behavior by the receiver's contract:
    /// - If the receiver contract promise fails or returns invalid value, the full transfer amount should be refunded.
    /// - If the receiver contract overspent the tokens, and the `receiver_id` balance is lower than the required refund
    /// amount, the remaining balance should be refunded. See Security section of the standard.
    ///
    /// Both accounts should be registered with the contract for transfer to succeed.
    /// Method is required to be able to accept attached deposits - to not panic on attached deposit. See Security
    /// section of the standard.
    ///
    /// Arguments:
    /// - `receiver_id` - the account ID of the receiver contract. This contract will be called.
    /// - `amount` - the amount of tokens to transfer. Should be a positive number in decimal string representation.
    /// - `msg` - a string message that will be passed to `on_ft_receive` contract call.
    /// - `memo` - an optional string field in a free form to associate a memo with this transfer.
    /// Returns a promise to resolve transfer call which will return the used amount (see suggested trait to resolve
    /// transfer).
    /// #[payable]
    fn ft_transfer(
        &mut self,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
        memo: Option<String>,
        data: Option<String>,
    );

    /// #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
        memo: Option<String>,
        data: Option<String>,
    ) -> Promise;

    fn ft_confirm_transfer(
        &mut self,
        recipient: ValidAccountId,
        amount: TokenAmount,
        memo: Option<String>,
        data: Option<String>,
    ) -> Promise;

    fn ft_transfer_with_vault(
        &mut self,
        recipient: ValidAccountId,
        amount: TokenAmount,
        memo: Option<String>,
        data: Option<String>,
    ) -> Promise;

    fn ft_total_supply(&self) -> TokenAmount;

    fn ft_balance_of(&self, account_id: ValidAccountId) -> TokenAmount;
}

/// Receiver of the Fungible Token for `ft_transfer_and_notify` calls.
pub trait FungibleTokenReceiver {
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: TokenAmount,
        data: Option<String>,
    );
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

impl Deref for TokenAmount {
    type Target = u128;

    fn deref(&self) -> &Self::Target {
        &self.0 .0
    }
}

impl DerefMut for TokenAmount {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0 .0
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

impl Ord for TokenAmount {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value().cmp(&other.value())
    }
}

impl Eq for TokenAmount {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn token_amount() {
        let mut amount = TokenAmount::from(100);
        *amount += 10;
        assert_eq!(*amount, 110_u128.into());
        assert_eq!(amount, TokenAmount::from(110));
    }
}
