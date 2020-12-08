use crate::near::storage_keys::{
    ACCOUNTS_KEY_PREFIX, REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX, STAKE_BATCH_RECEIPTS_KEY_PREFIX,
    UNCLAIMED_REDEEME_STAKE_BATCH_FUNDS_KEY_PREFIX, UNCLAIMED_STAKE_BATCH_FUNDS_KEY_PREFIX,
};
use crate::{
    core::Hash,
    domain::{
        Account, BatchId, RedeemStakeBatch, RedeemStakeBatchReceipt, StakeBatch, StakeBatchReceipt,
        StorageUsage, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear,
    },
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Accounts {
    accounts: LookupMap<Hash, Account>,
    count: u128,

    total_storage_escrow: TimestampedNearBalance,
    total_storage_usagee: StorageUsage,
    total_near: TimestampedNearBalance,
    total_stake: TimestampedStakeBalance,

    // used to generate new batch IDs
    // - the sequence is incremented to generate a new batch ID
    batch_id_sequence: BatchId,
    // when the batches are processed, receipts are created
    stake_batch: Option<StakeBatch>,
    redeem_stake_batch: Option<RedeemStakeBatch>,

    // after users have claimed all funds from a receipt, then the map will clean itself up by removing
    // the receipt from storage
    stake_batch_receipts: UnorderedMap<BatchId, StakeBatchReceipt>,
    redeem_stake_batch_receipts: UnorderedMap<BatchId, RedeemStakeBatchReceipt>,
}

impl Default for Accounts {
    fn default() -> Self {
        Self {
            accounts: LookupMap::new(ACCOUNTS_KEY_PREFIX.to_vec()),
            count: 0,
            total_storage_escrow: Default::default(),
            total_storage_usagee: Default::default(),
            total_near: Default::default(),
            total_stake: Default::default(),
            batch_id_sequence: Default::default(),
            stake_batch: None,
            redeem_stake_batch: None,
            stake_batch_receipts: UnorderedMap::new(STAKE_BATCH_RECEIPTS_KEY_PREFIX.to_vec()),
            redeem_stake_batch_receipts: UnorderedMap::new(
                REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX.to_vec(),
            ),
        }
    }
}

impl Accounts {
    pub fn get(&self, account_id: &Hash) -> Option<Account> {
        self.accounts.get(account_id)
    }

    pub fn insert(&mut self, account_id: &Hash, account: &Account) -> Option<Account> {
        self.accounts.insert(account_id, account)
    }

    pub fn remove(&mut self, account_id: &Hash) -> Option<Account> {
        self.accounts.remove(account_id)
    }

    pub fn count(&self) -> u128 {
        self.count
    }

    pub fn total_storage_escrow(&self) -> TimestampedNearBalance {
        self.total_storage_escrow
    }

    pub fn total_storage_usagee(&self) -> StorageUsage {
        self.total_storage_usagee
    }

    pub fn total_near(&self) -> TimestampedNearBalance {
        self.total_near
    }

    pub fn total_stake(&self) -> TimestampedStakeBalance {
        self.total_stake
    }
}
