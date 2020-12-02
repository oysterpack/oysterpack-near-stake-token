use crate::data::staking_pools::StakingPoolId;
use crate::data::{
    Hash, TimestampedBalance, ACCOUNTS_KEY_PREFIX, ACCOUNT_STAKE_BALANCES_KEY_PREFIX,
};
use near_sdk::collections::LookupMap;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedMap,
    Balance, StorageUsage,
};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Accounts {
    /// the account ID hash is used as the key to ensure the TRIE is balanced
    accounts: LookupMap<Hash, Account>,
    /// using u128 to make this future proof ... at least for the foreseeable future
    /// - use case: IOT, e.g. every device could potentially have its own account
    count: u128,
}

impl Accounts {
    pub fn get(&self, account_id: &str) -> Option<Account> {
        self.accounts.get(&account_id.into())
    }

    pub fn remove(&mut self, account_id: &str) {
        if self.accounts.remove(&account_id.into()).is_some() {
            self.count -= 1;
        }
    }

    /// inserts the account - replacing any previous account record
    /// - if the account is replaced, then the previous version is returned
    pub fn insert(&mut self, account_id: &str, account: &Account) {
        if self.accounts.insert(&account_id.into(), account).is_none() {
            self.count += 1;
        }
    }

    pub fn count(&self) -> u128 {
        self.count
    }
}

impl Default for Accounts {
    fn default() -> Self {
        Self {
            count: 0,
            accounts: LookupMap::new(ACCOUNTS_KEY_PREFIX.to_vec()),
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Account {
    storage_escrow: TimestampedBalance,
    storage_usage: StorageUsage,
    /// STAKE token balances per staking pool
    stake: UnorderedMap<StakingPoolId, StakeBalance>,
    /// funds that are available for withdrawal
    near: TimestampedBalance,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            storage_escrow: TimestampedBalance::default(),
            storage_usage: 0,
            near: TimestampedBalance::default(),
            stake: UnorderedMap::new(ACCOUNT_STAKE_BALANCES_KEY_PREFIX.to_vec()),
        }
    }
}

impl Account {
    /// Returns the number of STAKE tokens owned for the specified staking pool.
    /// Returns None, if no record exists for the staking pool.
    pub fn stake_balance(&self, staking_pool_id: &StakingPoolId) -> StakeBalance {
        self.stake
            .get(staking_pool_id)
            .unwrap_or_else(StakeBalance::default)
    }

    pub fn is_staked_with(&self, staking_pool_id: &StakingPoolId) -> bool {
        self.stake.get(staking_pool_id).is_some()
    }

    pub fn near_balance(&self) -> TimestampedBalance {
        self.near
    }

    pub fn has_funds(&self) -> bool {
        return self.near > 0 || !self.stake.is_empty();
    }

    pub fn apply_near_credit(&mut self, credit: Balance) {
        self.near.credit(credit)
    }

    pub fn apply_near_debit(&mut self, debit: Balance) {
        self.near.debit(debit)
    }

    pub fn storage_escrow(&self) -> TimestampedBalance {
        self.storage_escrow
    }

    pub fn storage_usage(&self) -> StorageUsage {
        self.storage_usage
    }

    pub fn apply_storage_usage_increase(
        &mut self,
        storage_usage: StorageUsage,
        storage_fee: Balance,
    ) {
        self.storage_usage += storage_usage;
        self.storage_escrow.credit(storage_fee);
    }

    pub fn apply_storage_usage_decrease(
        &mut self,
        storage_usage: StorageUsage,
        storage_fee: Balance,
    ) {
        self.storage_usage -= storage_usage;
        self.storage_escrow.debit(storage_fee);
    }

    /// updates balances to track `deposit_and_stake` requests that have been submitted to the
    /// staking pool
    pub fn apply_deposit_and_stake_activity(
        &mut self,
        staking_pool_id: &StakingPoolId,
        deposit: Balance,
    ) {
        let mut balance = self.stake_balance(staking_pool_id);
        balance.deposit_and_stake_activity_balance.credit(deposit);
        self.stake.insert(staking_pool_id, &balance);
    }

    /// updates balances when confirmation from the staking pool has been received that funds have
    /// been successfully staked
    /// - returns false if the account is not staking with specified staking pool
    /// - debits the staked deposit balance from [deposit_and_stake_activity_balance] and credits
    ///   the confirmed funds to [stake_token_balance]
    pub fn apply_deposit_and_stake_activity_success(
        &mut self,
        staking_pool_id: &StakingPoolId,
        stake_deposit: Balance,
    ) {
        let mut balance = self.stake_balance(staking_pool_id);
        balance
            .deposit_and_stake_activity_balance
            .debit(stake_deposit);
        balance.stake_token_balance.credit(stake_deposit);
        self.stake.insert(staking_pool_id, &balance);
    }

    /// update balances when the `deposit_and_stake` staking-pool contract function call resulted in
    /// a failure
    /// - the [deposit_and_stake_activity_balance] balance is debited and credited to [near] balance
    pub fn apply_deposit_and_stake_activity_failure(
        &mut self,
        staking_pool_id: &StakingPoolId,
        deposit: Balance,
    ) {
        if let Some(mut balance) = self.stake.get(staking_pool_id) {
            balance.deposit_and_stake_activity_balance.debit(deposit);
            self.stake.insert(staking_pool_id, &balance);
            self.near.credit(deposit);
        }
    }
}

/// tracks staked funds
#[derive(BorshSerialize, BorshDeserialize, Default, PartialEq, Debug)]
pub struct StakeBalance {
    /// - used to track funds that have been submitted to the staking pool to be deposited and staked
    /// - if balance > 0, then these funds are in flight and we are awaiting confirmation from the staking pool
    /// - when success confirmations are received then the balance is shifted over to stake
    deposit_and_stake_activity_balance: TimestampedBalance,

    /// confirmed funds that have been deposited and staked with the staking pool
    /// - STAKE token balance that is unlocked and can be transferred
    stake_token_balance: TimestampedBalance,

    /// - stake that is locked will be redeemed for NEAR at
    ///   the next unstaking cycle
    locked_stake_token_balance: TimestampedBalance,
}

impl StakeBalance {
    pub fn deposit_and_stake_activity_balance(&self) -> TimestampedBalance {
        self.deposit_and_stake_activity_balance
    }

    pub fn stake_token_balance(&self) -> TimestampedBalance {
        self.stake_token_balance
    }

    pub fn locked_stake_token_balance(&self) -> TimestampedBalance {
        self.locked_stake_token_balance
    }

    /// returns true is the account has any non-zero balances
    pub fn has_funds(&self) -> bool {
        self.deposit_and_stake_activity_balance > 0
            || self.stake_token_balance > 0
            || self.locked_stake_token_balance > 0
    }
}
