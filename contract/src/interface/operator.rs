use crate::{config, interface::model::contract_state::ContractState};
use near_sdk::Promise;

/// provides functions to support DevOps
pub trait Operator {
    /// returns the contract's state
    /// - useful for monitoring and debugging
    fn contract_state(&self) -> ContractState;

    fn config(&self) -> config::Config;

    /// unlocks the contract
    ///
    /// ## Panics
    /// if not invoked by self as callback or the operator account
    fn release_run_stake_batch_lock(&mut self);

    /// if the [RedeemLock] state is unstaking, then clear it
    ///
    /// ## Panics
    /// if not invoked by self as callback or the operator account
    fn release_run_redeem_stake_batch_unstaking_lock(&mut self);

    /// submits a request to the staking pool to try to withdraw all available unstaked NEAR
    ///
    /// ## Panics
    /// if not invoked by self as callback or the operator account
    fn withdraw_all_funds_from_staking_pool(&self) -> Promise;
}
