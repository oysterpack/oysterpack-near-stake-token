use crate::interface::YoctoNear;
use near_sdk::{json_types::ValidAccountId, serde::Serialize};

#[derive(Serialize, Default, PartialEq, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct AccountStorageBalance {
    pub total: YoctoNear,
    /// represents portion of the account's total balance that is available for withdrawal
    pub available: YoctoNear,
}

/// Account Storage Standard API - NEP-145
///
/// On NEAR, the contract is responsible to pay for its long term persistent storage. Thus, multi-user contracts should be designed to pass on storage costs to its user accounts. The account storage API provides the following:
/// 1. Accounts can lookup the minimum required account storage balance for the initial deposit in order to be able to use the contract.
/// 2. Accounts can deposit NEAR funds into the contract to pay for storage for either itself or on behalf of another account. The initial deposit for the account must be at least the minimum amount required by the contract.
/// 3. Account storage balances can be looked up. The amount required to pay for the account's storage usage will be locked up in the contract. Any storage balance above storage staking costs is available for withdrawal.
/// 4. Accounts can withdraw NEAR from the account's storage available balance.
///
/// ### NOTES
/// - Use [unregister_account()][crate::AccountManagement::unregister_account] to close the account and withdraw all funds.
/// - STAKE contract accounts for changes in price for storage on the NEAR blockchain over time.
pub trait AccountStorage {
    /// Used by accounts to deposit funds to pay for account storage staking fees when registering the account.
    /// This function supports 2 deposit modes:
    ///
    /// 1. **self deposit** (`account_id` is not specified): predecessor account is used as the account
    /// 2. **third party deposit** (`account_id` is valid NEAR account ID):  the function caller is
    ///    depositing NEAR funds for the specified `account_id`
    ///
    /// If this is the initial deposit for the account, then the deposit must be enough to cover the minimum required balance.
    /// If the attached deposit is more than the required minimum balance, then the funds are credited to the account storage available balance.
    ///
    ///
    /// ##### Arguments
    /// - `account_id` - optional NEAR account ID. If not specified, then predecessor account ID will be used.
    ///
    /// ##### Returns
    /// The account's updated storage balance.
    ///
    /// ##### Panics
    /// - If the attached deposit is less than the minimum required account storage fee on the initial deposit.
    /// - If `account_id` is not a valid NEAR account ID
    ///
    /// `#[payable]`
    fn storage_deposit(&mut self, account_id: Option<ValidAccountId>) -> AccountStorageBalance;

    /// Used to withdraw NEAR from the predecessor account's storage available balance.
    /// If amount is not specified, then all of the account's storage available balance will be withdrawn.
    ///
    /// The attached yoctoNEAR will be refunded with the withdrawal transfer.
    ///
    /// The account is required to attach exactly 1 yoctoNEAR to the function call to prevent
    /// restricted function-call access-key calls.
    ///
    /// ##### Arguments
    /// - `amount` - the amount to withdraw from the account's storage available balance expressed in yoctoNEAR
    ///
    /// ##### Returns
    /// The account's updated storage balance.
    ///
    /// ##### Panics
    /// - If the attached deposit does not equal 1 yoctoNEAR
    /// - If the account is not registered with the contract
    /// - If the specified withdrawal amount is greater than the account's available storage balance
    fn storage_withdraw(&mut self, amount: Option<YoctoNear>) -> AccountStorageBalance;

    /// Used to look up the minimum balance required for the initial deposit.
    fn storage_minimum_balance(&self) -> YoctoNear;

    /// Used to lookup the account storage balance for the specified account.
    /// If the account is unknown to the contract then the total account storage balance returned will be zero.
    ///
    /// ##### Panics
    /// - If `account_id` is not a valid NEAR account ID
    fn storage_balance_of(&self, account_id: ValidAccountId) -> AccountStorageBalance;
}
