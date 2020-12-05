use crate::domain::YoctoNearValue;
use near_sdk::{
    json_types::{ValidAccountId, U128},
    serde::{Deserialize, Serialize},
};

pub trait AccountRegistry {
    fn account_registered(&self, account_id: ValidAccountId) -> bool;

    /// If no account exists for the predecessor account ID, then a new one is created and registered.
    /// The account is required to pay for its storage. Storage fees will be escrowed and refunded
    /// when the account is unregistered.
    ///
    /// Returns false if the account is already registered.
    /// If the account is already registered, then the deposit is refunded.
    ///
    /// #[payable]
    /// - account must pay for its storage
    /// - storage fee: ??? yoctoNEAR
    ///
    /// ## Panics
    /// - if deposit is not enough to cover storage fees
    /// - is account is already registered
    ///
    /// NOTE: panic will automatically refund any attached deposit
    fn register_account(&mut self) -> YoctoNearValue;

    /// An account can only be unregistered if the account has zero token balance, i.e., zero STAKE
    /// and NEAR balances. In order to unregister the account all NEAR must be unstaked and withdrawn
    /// from the account.
    ///
    /// If success, then returns the storage escrow fees that were refunded
    fn unregister_account(&mut self) -> Result<YoctoNearValue, UnregisterAccountFailure>;

    fn registered_accounts_count(&self) -> U128;
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum UnregisterAccountFailure {
    NotRegistered,
    /// account must first redeem and withdraw all funds before being able to unregister the account
    AccountHasFunds,
}
