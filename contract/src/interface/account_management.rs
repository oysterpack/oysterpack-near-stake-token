use crate::interface::{StakeAccount, YoctoNear};
use near_sdk::json_types::{ValidAccountId, U128};

/// Used to manage user accounts. The main use cases supported by this interface are:
/// 1. Users can register with the contract. Users are required to pay for account storage usage at
///    time of registration. Accounts are required to register in order to use the contract.
/// 2. Users can unregister with the contract. When a user unregisters, the account storage usage fee
///    will be refunded.
/// 3. The total number of registered users is tracked.
/// 4. Users can withdraw unstaked NEAR from STAKE that has been redeemed.
/// 5. User account info can be looked up.
pub trait AccountManagement {
    /// Creates and registers a new account for the predecessor account ID.
    /// - the account is required to pay for its storage. Storage fees will be escrowed and then refunded
    ///   when the account is unregistered - use [account_storage_escrow_fee](crate::interface::AccountManagement::account_storage_fee)
    ///   to lookup the required storage fee amount. Overpayment of storage fee is refunded.
    ///
    /// Gas Requirements: 4.5 TGas
    ///
    /// ## Panics
    /// - if deposit is not enough to cover storage usage fees
    /// - if account is already registered
    fn register_account(&mut self);

    /// In order to unregister the account all NEAR must be unstaked and withdrawn from the account.
    /// The escrowed storage fee will be refunded to the account.
    ///
    /// Gas Requirements: 8 TGas
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if registered account has funds
    fn unregister_account(&mut self);

    /// Returns the required deposit amount that is required for account registration.
    ///
    /// Gas Requirements: 3.5 TGas
    fn account_storage_fee(&self) -> YoctoNear;

    /// returns true if the account is registered
    ///
    /// Gas Requirements: 4 TGas
    fn account_registered(&self, account_id: ValidAccountId) -> bool;

    /// returns the total number of accounts that are registered with this contract
    fn total_registered_accounts(&self) -> U128;

    /// looks up the registered account
    ///
    /// Gas Requirements: 4 TGas
    fn lookup_account(&self, account_id: ValidAccountId) -> Option<StakeAccount>;
}
