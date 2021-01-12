use crate::interface::YoctoNear;
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct OwnerBalance {
    /// owner balance that is available for withdrawal
    pub available_balance: YoctoNear,
    /// total contract account balance
    pub contract_account_balance: YoctoNear,
    /// unstaked NEAR that is owned by user accounts
    pub user_accounts_near_balance: YoctoNear,
    pub near_liquidity: YoctoNear,
    /// amount of NEAR that has been deposited into STAKE batches
    pub customer_batched_stake_deposits: YoctoNear,
    ///
    pub total_account_storage_escrow: YoctoNear,
    pub contract_storage_usage_cost: YoctoNear,
}

impl OwnerBalance {
    pub fn value(&self) -> u128 {
        self.available_balance.value()
    }
}
