use crate::interface::{BlockHeight, ContractBalances, StorageUsage};
use crate::{
    domain::RedeemLock,
    interface::{
        BatchId, BlockTimeHeight, RedeemStakeBatch, RedeemStakeBatchReceipt, StakeBatch,
        StakeTokenValue, TimestampedNearBalance, TimestampedStakeBalance,
    },
};
use near_sdk::{
    json_types::U128,
    serde::{Deserialize, Serialize},
    AccountId,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractState {
    pub block: BlockTimeHeight,
    pub config_change_block_height: BlockHeight,

    pub staking_pool_id: AccountId,

    pub registered_accounts_count: U128,

    pub total_unstaked_near: TimestampedNearBalance,
    pub total_stake_supply: TimestampedStakeBalance,

    /// STAKE token value snapshot that was last taken when processing a batch
    pub stake_token_value: StakeTokenValue,

    pub batch_id_sequence: BatchId,

    pub stake_batch: Option<StakeBatch>,
    pub next_stake_batch: Option<StakeBatch>,

    pub redeem_stake_batch: Option<RedeemStakeBatch>,
    pub next_redeem_stake_batch: Option<RedeemStakeBatch>,
    pub pending_withdrawal: Option<RedeemStakeBatchReceipt>,

    pub run_stake_batch_locked: bool,
    pub run_redeem_stake_batch_lock: Option<RedeemLock>,

    pub balances: ContractBalances,
    /// total contract storage usage = [initial_storage_usage](ContractState::initial_storage_usage) + [storage_usage_growth](ContractState::storage_usage_growth)
    pub initial_storage_usage: StorageUsage,
    /// how much storage usage has grown since the contract was deployed
    /// - contract storage should be covered by the account storage fees that are escrowed
    pub storage_usage_growth: StorageUsage,
}
