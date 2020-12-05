use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Account {
    // storage_escrow: TimestampedBalance,
// storage_usage: StorageUsage,
//
// near: TimestampedBalance,
// stake: TimestampedBalance,
// unstaked: Vec<UnstakedBatch>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct UnstakedBatch {
    // batch_id: u64,
// balance: TimestampedBalance,
}
