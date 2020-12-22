use crate::interface::YoctoNear;
use near_sdk::json_types::ValidAccountId;
use near_sdk::AccountId;

pub trait ContractOwner {
    fn owner_id(&self) -> AccountId;

    /// returns the contract's NEAR balance that is owned
    /// - contract transaction fee rewards
    /// - staking fractional remainder from staking pool - when NEAR funds are staked with the staking pool,
    ///   the staking pool converts the NEAR funds to whole shares internally, i.e., fractional shares
    ///   are not possible. When funds are withdrawan, the fractional NEAR that was not staked is swept
    ///   into the contract account balance.
    fn owner_balance(&self) -> YoctoNear;

    /// TODO: need to protect against accounts that do not exist - options are
    ///       - send 1 yocto and transfer ownership only if NEAR transfer succeeds
    ///       - require a contract interface on the owner account
    ///
    /// ## Panics
    /// - if the predecessor account is not the owner account
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
    fn withdraw_owner_balance(&mut self, amount: YoctoNear) -> YoctoNear;
}
