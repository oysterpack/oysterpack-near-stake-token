pub trait Operator {
    /// unlocks the contract
    /// - can only be invoked by the contract or the operator account
    fn unlock(&mut self);
}
