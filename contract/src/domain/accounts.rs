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

    /// total available NEAR balance across all accounts
    total_near: TimestampedNearBalance,
    /// total STAKE token supply
    total_stake: TimestampedStakeBalance,

    /// used to generate new batch IDs
    /// - the sequence is incremented to generate a new batch ID
    /// - sequence ID starts at 1
    batch_id_sequence: BatchId,
    /// when the batches are processed, receipts are created
    stake_batch: Option<StakeBatch>,
    redeem_stake_batch: Option<RedeemStakeBatch>,

    /// after users have claimed all funds from a receipt, then the map will clean itself up by removing
    //. the receipt from storage
    stake_batch_receipts: UnorderedMap<BatchId, StakeBatchReceipt>,
    redeem_stake_batch_receipts: UnorderedMap<BatchId, RedeemStakeBatchReceipt>,
}

impl Default for Accounts {
    fn default() -> Self {
        Self {
            accounts: LookupMap::new(ACCOUNTS_KEY_PREFIX.to_vec()),
            count: 0,
            total_storage_escrow: Default::default(),
            total_near: Default::default(),
            total_stake: Default::default(),
            batch_id_sequence: BatchId(1),
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
    /// this is used to compute the storage usage fees to charge for account registration
    /// - the account is responsible to pay for its storage fees - account storage is allocated, measured,
    ///   and then freed
    pub(crate) fn allocate_account_template_to_measure_storage_usage(&mut self) {
        let hash = Hash::from([0u8; 32]);
        self.insert(&hash, &Account::account_template_to_measure_storage_usage());

        let batch_id = BatchId(0);
        self.stake_batch_receipts
            .insert(&batch_id, &StakeBatchReceipt::default());
        self.redeem_stake_batch_receipts
            .insert(&batch_id, &RedeemStakeBatchReceipt::default());
    }

    pub(crate) fn deallocate_account_template_to_measure_storage_usage(&mut self) {
        let hash = Hash::from([0u8; 32]);
        self.remove(&hash);

        let batch_id = BatchId(0);
        self.stake_batch_receipts.remove(&batch_id);
        self.redeem_stake_batch_receipts.remove(&batch_id);
    }

    pub fn get(&self, account_id: &Hash) -> Option<Account> {
        self.accounts.get(account_id)
    }

    pub fn insert(&mut self, account_id: &Hash, account: &Account) -> Option<Account> {
        match self.accounts.insert(account_id, account) {
            None => {
                self.count += 1;
                None
            }
            Some(previous) => Some(previous),
        }
    }

    pub fn remove(&mut self, account_id: &Hash) -> Option<Account> {
        match self.accounts.remove(account_id) {
            None => None,
            Some(account) => {
                self.count -= 1;
                Some(account)
            }
        }
    }

    pub fn count(&self) -> u128 {
        self.count
    }

    pub fn total_storage_escrow(&self) -> TimestampedNearBalance {
        self.total_storage_escrow
    }

    pub fn total_near(&self) -> TimestampedNearBalance {
        self.total_near
    }

    pub fn total_stake(&self) -> TimestampedStakeBalance {
        self.total_stake
    }

    pub fn stake_batch(&self) -> Option<StakeBatch> {
        self.stake_batch
    }

    pub fn redeem_stake_batch(&self) -> Option<RedeemStakeBatch> {
        self.redeem_stake_batch
    }

    pub fn credit_stake_batch(&mut self, amount: YoctoNear) {
        let batch = match self.stake_batch {
            None => {
                *self.batch_id_sequence += 1;
                StakeBatch::new(self.batch_id_sequence, amount)
            }
            Some(mut batch) => {
                batch.add(amount);
                batch
            }
        };
        self.stake_batch = Some(batch)
    }

    pub fn debit_stake_batch(&mut self, amount: YoctoNear) {
        if let Some(mut batch) = self.stake_batch {
            batch.remove(amount);
            if batch.balance() == 0 {
                self.stake_batch = None
            }
        }
    }
}
