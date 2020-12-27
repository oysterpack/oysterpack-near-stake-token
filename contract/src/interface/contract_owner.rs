use crate::interface::YoctoNear;
use near_sdk::json_types::ValidAccountId;
use near_sdk::AccountId;

pub trait ContractOwner {
    fn owner_id(&self) -> AccountId;

    /// returns the contract's NEAR balance that is owned and available for withdrawal minus 1 NEAR
    ///
    /// ## Notes
    /// - owner balance accumulates contract transaction fee rewards
    /// - 1 NEAR is left behind as a safety measure to make sure the contract has enough balance to
    ///   function
    ///
    /// <pre>
    /// balance computation = env::account_balance()
    ///   - total_customer_accounts_unstaked_balance
    ///   - customer_batched_stake_deposits
    ///   - total_account_storage_escrow
    ///   - contract_storage_usage_cost
    ///   - 1 NEAR
    /// </pre>
    fn owner_balance(&self) -> YoctoNear;

    /// The new owner must have a registered account to protect against accounts that do not exist
    ///
    /// ## Panics
    /// - if the predecessor account is not the owner account
    /// - new owner account must be registered
    fn transfer_ownership(&mut self, new_owner: ValidAccountId);

    /// Deposits the owner's balance into the owners STAKE account
    ///
    /// NOTE: contract owner will need to register his account beforehand
    ///
    /// ## Panics
    /// - panics if the owner does not have a registered account
    /// - if the predecessor account is not the owner account
    fn stake_all_owner_balance(&mut self) -> YoctoNear;

    /// Deposits the owner's balance into the owners STAKE account
    ///
    /// ## Panics
    /// - panics if the owner does not have a registered account
    /// - if the owner balance is too low to fulfill the request
    /// - if the predecessor account is not the owner account
    fn stake_owner_balance(&mut self, amount: YoctoNear);

    /// transfers the entire owner balance to the owner's account
    ///
    /// # Panics
    /// - if the predecessor account is not the owner account
    /// if owner account balance is zero
    fn withdraw_all_owner_balance(&mut self) -> YoctoNear;

    /// transfers the entire owner balance to the owner's account
    ///
    /// ## Panics
    /// - panics if the owner does not have a registered account
    /// - if the owner balance is too low to fulfill the request
    /// - if the predecessor account is not the owner account
    fn withdraw_owner_balance(&mut self, amount: YoctoNear);
}
