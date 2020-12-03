//! This module is provides object storage on the NEAR blockchain for [Account] objects.

use crate::data::staking_pools::StakingPoolId;
use crate::data::{
    Hash, TimestampedBalance, ACCOUNTS_KEY_PREFIX, ACCOUNT_STAKE_BALANCES_KEY_PREFIX,
};
use near_sdk::collections::LookupMap;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedMap,
    env, Balance, StorageUsage,
};

/// Accounts provides key-value persistent storage [Account] objects on the NEAR blockchain:
///
/// [AccountId] -> [Account]
///
/// [Account]: crate::data::accounts::Account
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Accounts {
    /// the account ID hash is used as the key to ensure the TRIE is balanced
    accounts: LookupMap<Hash, Account>,
    /// using u128 to make this future proof ... at least for the foreseeable future
    /// - use case: IOT, e.g. every device could potentially have its own account
    count: u128,
}

impl Accounts {
    /// reads the object from persistent storage on the NEAR
    ///
    /// NOTE: in order to ensure that all [Account] state is persistently stored, use [insert]
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
    ///
    /// NOTE: this will persist the object on the NEAR blockchain
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
    /// Returns the STAKE balance for the specified staking pool
    pub fn stake_balance(&self, staking_pool_id: &StakingPoolId) -> StakeBalance {
        self.stake
            .get(staking_pool_id)
            .unwrap_or_else(StakeBalance::default)
    }

    pub fn is_staked_with(&self, staking_pool_id: &StakingPoolId) -> bool {
        self.stake.get(staking_pool_id).is_some()
    }

    pub fn near_balance(&self) -> &TimestampedBalance {
        &self.near
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

    pub fn storage_escrow(&self) -> &TimestampedBalance {
        &self.storage_escrow
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

    /// locks STAKE tokens, which will be unstaked during next unstaking cycle
    ///
    /// ## Panics
    /// if the account does not have enough STAKE balance to satisfy the request
    pub fn unstake(&mut self, staking_pool_id: &StakingPoolId, stake_token_amount: Balance) {
        match self.stake.get(staking_pool_id) {
            Some(mut balance) => {
                assert!(
                    balance.stake_token_balance >= stake_token_amount,
                    "the account STAKE balance is too low to fulfill the unstake request for staking pool: {}", 
                    staking_pool_id,
                );
                balance.stake_token_balance.debit(stake_token_amount);
                balance
                    .locked_stake_token_balance
                    .credit(stake_token_amount);
                self.stake.insert(staking_pool_id, &balance);
            }
            None => panic!(
                "unstake request failed because STAKE balance is 0 for {}",
                staking_pool_id,
            ),
        }
    }

    /// Returns how many STAKE tokens were unstaked.
    ///
    /// The STAKE tokens will be scheduled to be unstaked with the staking pool on the next unstaking cycle.
    pub fn unstake_all(&mut self, staking_pool_id: &StakingPoolId) -> Balance {
        match self.stake.get(staking_pool_id) {
            Some(mut balance) => {
                let stake_token_balance = balance.stake_token_balance.balance;
                if stake_token_balance == 0 {
                    return 0;
                }
                balance.stake_token_balance.debit(stake_token_balance);
                balance
                    .locked_stake_token_balance
                    .credit(stake_token_balance);
                self.stake.insert(staking_pool_id, &balance);
                stake_token_balance
            }
            None => 0,
        }
    }

    /// restakes tokens that are currently unstaked, i.e., locked and scheduled to be unstaked
    /// - this enables the user to change the number of STAKE tokens to unstake
    pub fn restake_locked_stake(
        &mut self,
        staking_pool_id: &StakingPoolId,
        stake_token_amount: Balance,
    ) {
        match self.stake.get(staking_pool_id) {
            Some(mut balance) => {
                assert!(
                    balance.locked_stake_token_balance >= stake_token_amount,
                    "the account locked STAKE balance is too low to fulfill the restake request for staking pool: {}",
                    staking_pool_id,
                );
                balance.stake_token_balance.credit(stake_token_amount);
                balance.locked_stake_token_balance.debit(stake_token_amount);
            }
            None => panic!(
                "restake request failed because account has no STAKE with staking pool: {}",
                staking_pool_id,
            ),
        }
    }

    pub fn restake_all_locked_stake(&mut self, staking_pool_id: &StakingPoolId) -> Balance {
        match self.stake.get(staking_pool_id) {
            Some(mut balance) => {
                let stake_token_balance = balance.locked_stake_token_balance.balance;
                if stake_token_balance == 0 {
                    return 0;
                }
                balance.stake_token_balance.credit(stake_token_balance);
                balance
                    .locked_stake_token_balance
                    .debit(stake_token_balance);
                stake_token_balance
            }
            None => 0,
        }
    }

    /// enables an account to stake NEAR from their NEAR balance
    pub fn stake_from_near_balance(
        &mut self,
        staking_pool_id: &StakingPoolId,
        near_token_amount: Balance,
    ) {
        assert!(
            self.near.balance >= near_token_amount,
            "the account NEAR balance is too low to fulfill the stake request",
        );
        self.apply_deposit_and_stake_activity(staking_pool_id, near_token_amount);
        self.near.debit(near_token_amount);
    }

    /// enables an account to stake NEAR from their NEAR balance
    ///
    /// Returns how much NEAR was staked
    pub fn stake_all_near_balance(
        &mut self,
        staking_pool_id: &StakingPoolId,
        near_token_amount: Balance,
    ) -> Balance {
        let near_balance = self.near.balance;
        if near_balance == 0 {
            return 0;
        }
        self.apply_deposit_and_stake_activity(staking_pool_id, near_balance);
        self.near.debit(near_balance);
        near_balance
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
    pub fn deposit_and_stake_activity_balance(&self) -> &TimestampedBalance {
        &self.deposit_and_stake_activity_balance
    }

    pub fn stake_token_balance(&self) -> &TimestampedBalance {
        &self.stake_token_balance
    }

    pub fn locked_stake_token_balance(&self) -> &TimestampedBalance {
        &self.locked_stake_token_balance
    }

    /// returns true is the account has any non-zero balances
    pub fn has_funds(&self) -> bool {
        self.deposit_and_stake_activity_balance > 0
            || self.stake_token_balance > 0
            || self.locked_stake_token_balance > 0
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::common::json_types::YoctoNEAR;
    use crate::common::YOCTO;
    use crate::test_utils::near::new_context;
    use near_sdk::{testing_env, MockedBlockchain, VMContext};

    #[test]
    fn accounts_crud() {
        let account_id = "bob.near".to_string();
        let context = new_context(account_id.clone());
        testing_env!(context);

        let mut accounts = Accounts::default();
        assert_eq!(accounts.count(), 0);
        assert!(accounts.get(&account_id).is_none());

        // insert
        let account = Account::default();
        accounts.insert(&account_id, &account);
        assert_eq!(accounts.count(), 1);
        assert!(accounts.get(&account_id).is_some());

        // re-insert the same account
        accounts.insert(&account_id, &account);
        assert_eq!(accounts.count(), 1);
        assert!(accounts.get(&account_id).is_some());

        // insert a second account
        accounts.insert("alice.near", &account);
        assert_eq!(accounts.count(), 2);
        assert!(accounts.get("alice.near").is_some());

        // remove account
        accounts.remove(&account_id);
        assert_eq!(accounts.count(), 1);
        assert!(accounts.get(&account_id).is_none());
    }

    #[test]
    fn account_stake_success() {
        let account_id = "bob.near".to_string();
        let context = new_context(account_id.clone());
        testing_env!(context);

        let staking_pool_id: StakingPoolId = "staking-pool.near".to_string();
        // Given a new account is registered
        let mut accounts = Accounts::default();
        accounts.insert(&account_id, &Account::default());

        // Then it will have zero funds to begin with
        let mut account = accounts.get(&account_id).unwrap();
        assert!(!account.has_funds());
        assert!(!account.stake_balance(&staking_pool_id).has_funds());

        // account stakes some funds ...
        account.apply_deposit_and_stake_activity(&staking_pool_id, 100 * YOCTO);
        {
            let account = accounts.get(&account_id).unwrap();
            assert!(
                !account.has_funds(),
                "state changes should not be persisted - in order to persist state, \
                the account object needs to be written to storage via Accounts::insert"
            );
        }
        // persist state changes
        accounts.insert(&account_id, &account);
        let mut account = accounts.get(&account_id).unwrap();
        assert!(account.has_funds());
        let stake_balance = account.stake_balance(&staking_pool_id);
        assert_eq!(
            stake_balance.deposit_and_stake_activity_balance().balance,
            100 * YOCTO
        );
        assert_eq!(stake_balance.stake_token_balance().balance, 0);
        assert_eq!(stake_balance.locked_stake_token_balance().balance, 0);

        // stake is confirmed ...
        account.apply_deposit_and_stake_activity_success(&staking_pool_id, 100 * YOCTO);
        accounts.insert(&account_id, &account);
        let account = accounts.get(&account_id).unwrap();
        assert!(account.has_funds());
        let stake_balance = account.stake_balance(&staking_pool_id);
        assert_eq!(
            stake_balance.deposit_and_stake_activity_balance().balance,
            0
        );
        assert_eq!(stake_balance.stake_token_balance().balance, 100 * YOCTO);
        assert_eq!(stake_balance.locked_stake_token_balance().balance, 0);
    }

    #[test]
    fn account_stake_failure() {
        let account_id = "bob.near".to_string();
        let context = new_context(account_id.clone());
        testing_env!(context);

        let staking_pool_id: StakingPoolId = "staking-pool.near".to_string();
        // Given a new account is registered
        let mut accounts = Accounts::default();
        accounts.insert(&account_id, &Account::default());

        // Then it will have zero funds to begin with
        let mut account = accounts.get(&account_id).unwrap();
        assert!(!account.has_funds());
        assert!(!account.is_staked_with(&staking_pool_id));

        // account stakes some funds ...
        account.apply_deposit_and_stake_activity(&staking_pool_id, 100 * YOCTO);
        {
            let account = accounts.get(&account_id).unwrap();
            assert!(
                !account.has_funds(),
                "state changes should not be persisted - in order to persist state, \
                the account object needs to be written to storage via Accounts::insert"
            );
        }
        // persist state changes
        accounts.insert(&account_id, &account);
        let mut account = accounts.get(&account_id).unwrap();
        assert!(account.has_funds());
        let stake_balance = account.stake_balance(&staking_pool_id);
        assert_eq!(
            stake_balance.deposit_and_stake_activity_balance().balance,
            100 * YOCTO
        );
        assert_eq!(stake_balance.stake_token_balance().balance, 0);
        assert_eq!(stake_balance.locked_stake_token_balance().balance, 0);

        // staking pool request failed ...
        account.apply_deposit_and_stake_activity_failure(&staking_pool_id, 100 * YOCTO);
        accounts.insert(&account_id, &account);
        let account = accounts.get(&account_id).unwrap();
        assert!(account.has_funds());
        let stake_balance = account.stake_balance(&staking_pool_id);
        assert_eq!(
            stake_balance.deposit_and_stake_activity_balance().balance,
            0
        );
        assert_eq!(stake_balance.stake_token_balance().balance, 0);
        assert_eq!(stake_balance.locked_stake_token_balance().balance, 0);
        assert_eq!(account.near_balance().balance, 100 * YOCTO);
    }

    #[test]
    fn unstake() {
        let account_id = "bob.near".to_string();
        let context = new_context(account_id.clone());
        testing_env!(context);

        let staking_pool_id: StakingPoolId = "staking-pool.near".to_string();
        // Given a new account is registered
        let mut accounts = Accounts::default();
        accounts.insert(&account_id, &Account::default());

        let mut account = accounts.get(&account_id).unwrap();
        account.apply_deposit_and_stake_activity(&staking_pool_id, 1000);
        accounts.insert(&account_id, &account);

        let mut account = accounts.get(&account_id).unwrap();
        account.apply_deposit_and_stake_activity_success(&staking_pool_id, 1000);
        accounts.insert(&account_id, &account);

        let mut account = accounts.get(&account_id).unwrap();
        account.unstake(&staking_pool_id, 400);
        accounts.insert(&account_id, &account);

        let account = accounts.get(&account_id).unwrap();
        assert_eq!(
            account
                .stake_balance(&staking_pool_id)
                .locked_stake_token_balance()
                .balance,
            400
        );
        assert_eq!(
            account
                .stake_balance(&staking_pool_id)
                .stake_token_balance()
                .balance,
            600
        );
    }

    #[test]
    #[should_panic]
    fn unstake_balance_too_low() {
        let account_id = "bob.near".to_string();
        let context = new_context(account_id.clone());
        testing_env!(context);

        let staking_pool_id: StakingPoolId = "staking-pool.near".to_string();
        // Given a new account is registered
        let mut accounts = Accounts::default();
        accounts.insert(&account_id, &Account::default());

        let mut account = accounts.get(&account_id).unwrap();
        account.unstake(&staking_pool_id, 400);
    }

    #[test]
    fn unstake_all() {
        let account_id = "bob.near".to_string();
        let context = new_context(account_id.clone());
        testing_env!(context);

        let staking_pool_id: StakingPoolId = "staking-pool.near".to_string();
        // Given a new account is registered
        let mut accounts = Accounts::default();
        accounts.insert(&account_id, &Account::default());

        let mut account = accounts.get(&account_id).unwrap();
        account.apply_deposit_and_stake_activity(&staking_pool_id, 1000);
        accounts.insert(&account_id, &account);

        let mut account = accounts.get(&account_id).unwrap();
        account.apply_deposit_and_stake_activity_success(&staking_pool_id, 1000);
        accounts.insert(&account_id, &account);

        let mut account = accounts.get(&account_id).unwrap();
        assert_eq!(account.unstake_all(&staking_pool_id), 1000);
        accounts.insert(&account_id, &account);

        let account = accounts.get(&account_id).unwrap();
        assert_eq!(
            account
                .stake_balance(&staking_pool_id)
                .locked_stake_token_balance()
                .balance,
            1000
        );
        assert_eq!(
            account
                .stake_balance(&staking_pool_id)
                .stake_token_balance()
                .balance,
            0
        );
    }
}
