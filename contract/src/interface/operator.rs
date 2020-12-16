pub trait Operator {
    /// unlocks the contract
    /// - can only be invoked by the contract or the operator account
    fn release_run_stake_batch_lock(&mut self);
}
