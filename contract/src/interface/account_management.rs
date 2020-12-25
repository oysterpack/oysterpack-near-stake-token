use crate::interface::{StakeAccount, YoctoNear};
use near_sdk::json_types::{ValidAccountId, U128};

/// Accounts are required to register in order to use the contract.
pub trait AccountManagement {
    /// Creates and registers a new account for the predecessor account ID.
    /// - the account is required to pay for its storage. Storage fees will be escrowed and then refunded
    ///   when the account is unregistered - use [account_storage_escrow_fee](crate::interface::AccountManagement::account_storage_escrow_fee)
    ///   to lookup the required storage fee amount. Overpayment of storage fee is refunded.
    ///
    /// ## Panics
    /// - if deposit is not enough to cover storage usage fees
    /// - if account is already registered
    fn register_account(&mut self);

    /// In order to unregister the account all NEAR must be unstaked and withdrawn from the account.
    /// The escrowed storage fee will be refunded to the account.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if registered account has funds
    fn unregister_account(&mut self);

    /// Returns the required deposit amount that is required for account registration.
    fn account_storage_fee(&self) -> YoctoNear;

    /// returns true if the account is registered
    fn account_registered(&self, account_id: ValidAccountId) -> bool;

    /// returns the total number of accounts that are registered with this contract
    fn total_registered_accounts(&self) -> U128;

    /// looks up the registered account
    fn lookup_account(&self, account_id: ValidAccountId) -> Option<StakeAccount>;

    /// Withdraws the specified amount from the account's available NEAR balance and transfers the
    /// funds to the account.
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are not enough available NEAR funds to fulfill the request
    fn withdraw(&mut self, amount: YoctoNear);

    /// Withdraws all available NEAR funds from the account and transfers the
    /// funds to the account.
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are no funds to withdraw
    fn withdraw_all(&mut self);
}
