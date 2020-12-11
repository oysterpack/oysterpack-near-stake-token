use crate::domain::YoctoNearValue;
use crate::interface::{StakeAccount, YoctoNear};
use near_sdk::{
    json_types::{ValidAccountId, U128},
    serde::{Deserialize, Serialize},
    Promise, PromiseOrValue,
};

pub trait AccountManagement {
    ////////////////////////////
    ///    CHANGE METHODS   ///
    //////////////////////////

    /// If no account exists for the predecessor account ID, then a new one is created and registered.
    /// The attached deposit will be staked minus the account storage fees.
    /// The account is required to pay for its storage. Storage fees will be escrowed and refunded
    /// when the account is unregistered.
    ///
    /// #[payable]
    /// - storage escrow fee is required
    ///   - use [account_storage_escrow_fee] to lookup the required storage fee amount
    /// - any amount above the storage fee will be staked
    ///
    /// ## Panics
    /// - if deposit is not enough to cover storage fees
    /// - is account is already registered
    ///
    /// ## NOTES
    /// - panic will automatically refund any attached deposit
    fn register_account(&mut self);

    /// In order to unregister the account all NEAR must be unstaked and withdrawn from the account.
    /// The escrowed storage fee will be refunded to the account.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if registered account has funds
    fn unregister_account(&mut self);

    ////////////////////////////
    ///     VIEW METHODS    ///
    //////////////////////////

    /// Returns the required deposit amount that is required for account registration.
    fn account_storage_fee(&self) -> YoctoNear;

    fn account_registered(&self, account_id: ValidAccountId) -> bool;

    fn total_registered_accounts(&self) -> U128;

    fn lookup_account(&self, account_id: ValidAccountId) -> Option<StakeAccount>;

    /// Withdraws the specified amount from the account's available NEAR balance and transfers the
    /// funds to the account.
    ///
    /// Returns the Promise that transfers the funds. This enables the client to react to the fund
    /// transfer.
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are not enough available NEAR funds to fulfill the request
    fn withdraw(&mut self, amount: YoctoNear) -> Promise;

    /// Withdraws all available NEAR funds from the account.
    ///
    /// Returns the Promise that transfers the funds. This enables the client to react to the fund
    /// transfer.
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are no funds to withdraw
    fn withdraw_all(&mut self) -> Promise;
}
