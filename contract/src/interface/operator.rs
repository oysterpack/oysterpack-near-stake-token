use crate::interface::{model::contract_state::ContractState, Config};
use near_sdk::Promise;

/// provides functions to support DevOps
pub trait Operator {
    /// returns the contract's state
    /// - useful for monitoring and debugging
    fn contract_state(&self) -> ContractState;

    fn config(&self) -> Config;

    /// resets the config to default settings
    ///
    /// ## Panics
    /// if not invoked by the operator account
    fn reset_config_default(&mut self) -> Config;

    /// merges in config changes
    /// - performs basic validation to prevent mis-configurations
    ///
    /// NOTE: you can [force a config change](Operator::force_update_config) if the validation logic
    ///       is flawed or becomes invalidated because of NEAR platform changes in the future.
    ///
    /// ## Panics
    /// - if not invoked by the operator account
    /// - if config validation fails
    fn update_config(&mut self, config: Config) -> Config;

    /// merges in config changes with no validations run
    /// - the purpose to allow config to be updated without validation is in case the assumptions
    ///   made for validation prove to be wrong later on, e.g, gas usage or storage fees may change
    ///   that require config changes that would cause validation to fail
    ///
    /// ## Panics
    /// - if not invoked by the operator account
    fn force_update_config(&mut self, config: Config) -> Config;

    /// unlocks the contract
    ///
    /// ## Panics
    /// if not invoked by self as callback or the operator account
    fn release_run_stake_batch_lock(&mut self);

    /// if the [RedeemLock](crate::domain::RedeemLock) state is unstaking, then clear it
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
