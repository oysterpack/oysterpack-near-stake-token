use crate::interface::model::contract_state::ContractState;
use near_sdk::Promise;

pub trait Operator {
    /// unlocks the contract
    /// - can only be invoked by the contract or the operator account
    fn release_run_stake_batch_lock(&mut self);

    /// if the [RedeemLock] state is unstaking, then clear it
    fn release_run_redeem_stake_batch_unstaking_lock(&mut self);

    /// submits a request to the staking pool to try to withdraw all available unstaked NEAR
    fn withdraw_all_funds_from_staking_pool(&self) -> Promise;

    fn contract_state(&self) -> ContractState;
}
