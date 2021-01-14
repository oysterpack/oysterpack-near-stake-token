use crate::interface::{ContractBalances, YoctoNear};

pub trait ContractFinancials {
    /// returns consolidated view of contract balances
    fn balances(&self) -> ContractBalances;

    /// NEAR funds that are deposited are added to the contract's STAKE fund, which will be staked
    /// to boost STAKE token value by increasing the staked NEAR balance.
    ///
    /// Returns the updated STAKE fund balance.
    ///
    /// NOTE: The STAKE funds will be staked the next time the [StakeBatch](crate::domain::StakeBatch) is run.
    ///
    /// #\[payable\]
    fn deposit_earnings(&mut self) -> YoctoNear;
}
