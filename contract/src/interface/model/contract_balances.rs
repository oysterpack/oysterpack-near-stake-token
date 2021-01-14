use crate::{
    domain,
    interface::{BlockHeight, BlockTimestamp, YoctoNear},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractBalances {
    pub total_contract_balance: YoctoNear,
    pub total_contract_storage_usage_cost: YoctoNear,
    /// `total_contract_balance` - `total_contract_storage_usage_cost`
    pub total_available_balance: YoctoNear,

    /// total portion of the contract balance that is owned by the registered user accounts
    pub total_user_accounts_balance: YoctoNear,
    /// amount of NEAR that has been deposited into STAKE batches
    pub customer_batched_stake_deposits: YoctoNear,
    /// amount of unstaked NEAR that has been withdrawn from the staking pool and available for
    /// withdrawal by the user accounts from the STAKE token contract
    pub total_available_unstaked_near: YoctoNear,
    /// amount of NEAR in the liquidity pool that user accounts can draw against to claim funds for
    /// [RedeemStakeBatchReceipts](crate::domain::RedeemStakeBatch)
    pub near_liquidity_pool: YoctoNear,
    /// total balance that has been escrowed to pay for user account storage
    pub total_account_storage_escrow: YoctoNear,

    /// contract earnings that have been accumulated but not yet staked
    ///
    /// NOTE: earnings are distributed when funds are staked, i.e.,
    ///       when [stake()](crate::interface::StakingService::stake) is run.
    pub contract_earnings: YoctoNear,
    /// percentage of contract_earnings that are owned by the contract owner
    pub contract_owner_earnings: YoctoNear,
    /// percentage of contract_earnings that are owned by the user accounts
    pub user_accounts_earnings: YoctoNear,

    /// funds that have been deposited for boosting staking, but not yet staked
    pub collected_earnings: YoctoNear,

    /// portion of the locked contract account balance that the contract owner is responsible for
    /// to pay for contract storage usage - based on the contract storage usage when first deployed
    pub contract_owner_storage_usage_cost: YoctoNear,
    /// balance that is currently available for the contract owner, which excludes [`ContractBalances::contract_owner_storage_usage_cost`]
    /// and the [`ContractBalances::contract_required_operational_balance`].
    /// - NOTE: accrued contract earnings are not applied until funds are staked
    pub contract_owner_available_balance: YoctoNear,

    /// the contract unlocked balance that is required to maintain the contract operational
    /// - if the contract balance falls below storage allocation costs, then the contract will not
    ///   be operational until more funds are deposited
    pub contract_required_operational_balance: YoctoNear,

    pub block_height: BlockHeight,
    pub block_timestamp: BlockTimestamp,
}

impl ContractBalances {
    pub fn owner_available_balance(&self) -> domain::YoctoNear {
        domain::YoctoNear(self.contract_owner_available_balance.value())
    }
}
