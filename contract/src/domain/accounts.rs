use crate::near::storage_keys::{
    ACCOUNTS_KEY_PREFIX, REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX, STAKE_BATCH_RECEIPTS_KEY_PREFIX,
    UNCLAIMED_REDEEME_STAKE_BATCH_FUNDS_KEY_PREFIX, UNCLAIMED_STAKE_BATCH_FUNDS_KEY_PREFIX,
};
use crate::{
    core::Hash,
    domain::{
        Account, BatchClaimTickets, BatchId, RedeemStakeBatch, RedeemStakeBatchReceipt, StakeBatch,
        StakeBatchReceipt, StorageUsage, TimestampedNearBalance, TimestampedStakeBalance,
        YoctoNear,
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

    // tracks users that have unclaimed funds
    // - this is required to defend against an attack vector where users stake small amounts and then
    //   don't claim the funds. This would result in storage increases at the contract's expense and
    //   eventually cause all contract funds to be locked up for storage - thus blocking future
    //   transactions until more NEAR funds are deposited
    // - when contract storage fees go above a threshold, the contract will start processing the claims
    //   for the users in order to free up storage
    //   - claim processing fees will be applied against the users esscrowed storage fees - the escrowed
    //     storage fees will be deducted from the user's contract as the claim processing fee
    unclaimed_stake_batch_funds: BatchClaimTickets,
    unclaimed_redeem_stake_batch_funds: BatchClaimTickets,
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
            unclaimed_stake_batch_funds: BatchClaimTickets::new(
                UNCLAIMED_STAKE_BATCH_FUNDS_KEY_PREFIX,
            ),
            unclaimed_redeem_stake_batch_funds: BatchClaimTickets::new(
                UNCLAIMED_REDEEME_STAKE_BATCH_FUNDS_KEY_PREFIX,
            ),
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
