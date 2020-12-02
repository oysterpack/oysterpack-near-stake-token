use crate::account::{Account, StakeBalance};
use crate::common::{
    assert_predecessor_is_self, is_promise_result_success,
    json_types::{self, YoctoNEAR, YoctoSTAKE},
    StakingPoolId, NO_DEPOSIT, YOCTO, ZERO_BALANCE,
};
use crate::data::TimestampedBalance;
use crate::StakeTokenService;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::{
    env, ext_contract,
    json_types::{U128, U64},
    near_bindgen,
    serde::{self, Deserialize, Serialize},
    AccountId, Balance, Promise,
};
use primitive_types::U256;
use std::collections::HashMap;

pub trait StakingService {
    /// Deposits the attached amount into the predecessor account and stakes it with the specified
    /// staking pool contract. Once the funds are successfully staked, then STAKE tokens are issued
    /// to the predecessor account.
    ///
    /// Promise returns [StakeReceipt]
    ///
    /// ## Account Storage Fees
    /// - any applicable storage fees will be deducted from the attached deposit
    /// - when staking to a new staking pool, then storage fees will be charged
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if not enough deposit was attached to cover storage fees
    fn deposit_and_stake(&mut self, staking_pool_id: StakingPoolId) -> Promise;

    /// Returns the number of staking pools that are staked with across all accounts.
    ///
    /// NOTE: there are STAKE tokens issued per staking pool
    fn staking_pool_count(&self) -> u32;

    /// Enables paging through the staking pool account IDs.
    /// - [start_index] defines the starting point for the iterator
    ///   - if the [start_index] is out of range, then an empty result set is returned, i.e., empty Vec
    /// - [max_results] defines the maximum number of results to return, i.e., page size
    fn staking_pool_ids(&self, start_index: u32, max_results: u32) -> StakingPoolIDs;

    /// Returns the number of STAKE tokens issued for the specified staking pool.
    fn stake_token_supply(&self, staking_pool: StakingPoolId) -> StakeSupply;

    /// Retrieves STAKE token supplies for a batch of staking pools
    fn stake_token_supply_batch(
        &self,
        staking_pools: Vec<StakingPoolId>,
    ) -> HashMap<StakingPoolId, StakeSupply>;

    /// Promise returns `Option<StakeTokenValue>`
    fn stake_token_value(&self, staking_pool_id: StakingPoolId) -> Promise;
}

/// Returns the STAKE token value at the specified block height.
/// STAKE token value is computed as [total_staked] / [total_supply]
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeTokenValue {
    pub token_supply: YoctoSTAKE,
    pub staked_balance: YoctoNEAR,
    pub block_height: json_types::BlockHeight,
    pub block_timestamp: json_types::BlockTimestamp,
}

impl StakeTokenValue {
    pub fn value(&self) -> YoctoNEAR {
        if self.staked_balance.0 == 0 || self.token_supply.0 == 0 {
            return YOCTO.into();
        }
        let value =
            U256::from(YOCTO) * U256::from(self.staked_balance.0) / U256::from(self.token_supply.0);
        value.as_u128().into()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolIDs {
    pub staking_pool_ids: Vec<StakingPoolId>,
    pub start_index: u32,
    pub staking_pool_total_count: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeSupply {
    pub supply: YoctoSTAKE,

    /// when the supply last changed
    pub block_timestamp: json_types::BlockTimestamp,
    pub block_height: json_types::BlockHeight,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeReceipt {
    /// amount in yoctoNEAR that was staked
    pub near_tokens_staked: YoctoNEAR,
    /// amount of yoctoSTAKE that was credited to the customer account
    pub stake_tokens_credit: YoctoSTAKE,
    /// amount of storage fees that were deducted
    ///
    /// ## Notes
    /// - storage fees will be charged when staking with a new staking pool
    pub storage_fees: Option<YoctoNEAR>,
}

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn deposit_and_stake(&mut self);

    fn get_account_staked_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn get_account_unstaked_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn get_account_total_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn withdraw_all(&mut self);

    fn unstake(&mut self, amount: json_types::Balance);
}

#[ext_contract(ext_staking_pool_callbacks)]
pub trait ExtStakingPoolCallbacks {
    /// ## failure handling
    /// - refund the attached deposit (stake_deposit + storage_fee)
    /// - if storage_fee > 0, then debit the storage fee escrow account
    ///   
    fn on_deposit_and_stake(
        &mut self,
        account_id: AccountId,
        stake_deposit: Balance,
        staking_pool_id: StakingPoolId,
        storage_fee: Balance,
    );

    fn on_get_account_staked_balance(
        &self,
        #[callback] balance: json_types::Balance,
        staking_pool_id: StakingPoolId,
    ) -> Option<StakeTokenValue>;

    fn on_get_account_unstaked_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn on_get_account_total_balance(&self, account_id: AccountId) -> json_types::Balance;

    fn on_withdraw_all(&mut self);

    fn on_unstake(&mut self, amount: json_types::Balance);
}

#[near_bindgen]
impl StakingService for StakeTokenService {
    /// Workflow:
    /// 1. assert that the predecessor account is registered
    /// 2. assert that there is an attached deposit
    /// 3. check if staking pool storage needs to be allocated for the account, i.e., is this is the
    ///    first time the account is staking with the specified pool
    ///    - if staking pool storage is allocated, the compute storage fees and deduct the storage
    ///      fees from the attached deposit. Then assert the remaining deposit is > 0.
    /// 4. track the pending activity
    /// 5. submit `deposit_and_stake` request to the staking pool
    /// 6. register a callback to handle the `deposit_and_stake` promise result
    #[payable]
    fn deposit_and_stake(&mut self, staking_pool_id: StakingPoolId) -> Promise {
        let mut account = self.expect_registered_predecessor_account();
        assert!(env::attached_deposit() > 0, "no deposit was attached");

        // If the account is not currently staking with the staking pool, then account storage
        // will need to be allocated to track the staking pool.
        let storage_fee = {
            let initial_storage_usage = env::storage_usage();
            if account.init_staking_pool(&staking_pool_id) {
                let storage_fee = self.assert_storage_fees(initial_storage_usage);
                let storage_usage = env::storage_usage() - initial_storage_usage;
                account.storage_usage_increased(storage_usage, storage_fee);
                storage_fee
            } else {
                ZERO_BALANCE
            }
        };
        // deduct storage fee from deposit
        let stake_deposit = env::attached_deposit() - storage_fee;
        assert!(
            stake_deposit > 0,
            "After deducting storage fees ({} yoctoNEAR) there was zero deposit to stake."
        );

        self.track_pending_deposit_and_stake_activity(
            &staking_pool_id,
            &mut account,
            stake_deposit,
        );

        // TODO:
        // if this is the first time we are using this staking pool, then we need to protect the contract
        // from a storage attack. An attacker can continue to submit staking requests against invalid
        // staking pools that do not exist. We only want to create storage for the staking pool if the
        // deposit_and_stake async function call succeeds.
        //
        // To be on the safe side, we should invoke every async function that this contract has
        // depends on to ensure the staking pool contract interface is compliant.
        //
        // Another option is give the user the option to verify that the staking pool is white-listed
        // by the NEAR foundation - via a different contact method (deposit_and_stake_with_whitelisted_pool)

        ext_staking_pool::deposit_and_stake(
            &staking_pool_id,
            stake_deposit,
            self.config.gas_config().deposit_and_stake(),
        )
        .then(ext_staking_pool_callbacks::on_deposit_and_stake(
            env::predecessor_account_id(),
            stake_deposit,
            staking_pool_id,
            storage_fee,
            // action receipt params
            &env::current_account_id(),
            NO_DEPOSIT,
            self.config.gas_config().on_deposit_and_stake(),
        ))
    }

    fn staking_pool_count(&self) -> u32 {
        unimplemented!()
    }

    fn staking_pool_ids(&self, start_index: u32, max_results: u32) -> StakingPoolIDs {
        unimplemented!()
    }

    fn stake_token_supply(&self, staking_pool: StakingPoolId) -> StakeSupply {
        unimplemented!()
    }

    fn stake_token_supply_batch(
        &self,
        staking_pools: Vec<StakingPoolId>,
    ) -> HashMap<StakingPoolId, StakeSupply> {
        unimplemented!()
    }

    fn stake_token_value(&self, staking_pool_id: StakingPoolId) -> Promise {
        unimplemented!()
    }
}

/// NEAR contract callbacks are all private, they should only be invoked by itself
#[near_bindgen]
impl StakeTokenService {
    pub fn on_deposit_and_stake(
        &mut self,
        account_id: AccountId,
        stake_deposit: Balance,
        staking_pool_id: StakingPoolId,
        storage_fee: Balance,
    ) {
        assert_predecessor_is_self();

        let mut account = self
            .accounts
            .get(&account_id)
            .expect(format!("account does not exist: {}", account_id).as_str());

        let success = is_promise_result_success(env::promise_result(0));
        self.track_completed_deposit_and_stake_activity(
            &staking_pool_id,
            &mut account,
            stake_deposit,
            success,
        );
        if !success {
            // refund deposit
            Promise::new(account_id).transfer(stake_deposit + storage_fee);
        }
    }
}

impl StakeTokenService {
    /// the pending activity is tracked at the staking pool and account level
    pub fn track_pending_deposit_and_stake_activity(
        &mut self,
        staking_pool_id: &StakingPoolId,
        account: &mut Account,
        stake_deposit: Balance,
    ) {
        {
            // track at the account level
            let mut staking_pool_balances = account
                .balances(&staking_pool_id)
                .expect("staking pool account balances should exist");
            staking_pool_balances
                .deposit_and_stake_activity
                .credit(stake_deposit);
            account.set_stake_balances(&staking_pool_id, &staking_pool_balances);
            self.accounts
                .upsert(&env::predecessor_account_id(), &account);
        }

        match self.deposit_and_stake_activity.get(staking_pool_id) {
            None => {
                self.deposit_and_stake_activity
                    .insert(staking_pool_id, &TimestampedBalance::new(stake_deposit));
            }
            Some(mut activity) => {
                activity.credit(stake_deposit);
                self.deposit_and_stake_activity
                    .insert(staking_pool_id, &activity);
            }
        }
    }

    pub fn track_completed_deposit_and_stake_activity(
        &mut self,
        staking_pool_id: &StakingPoolId,
        account: &mut Account,
        stake_deposit: Balance,
        success: bool,
    ) {
        // track at the account level
        {
            let mut staking_pool_balances = account
                .balances(&staking_pool_id)
                .expect("staking pool account balances should exist");
            staking_pool_balances
                .deposit_and_stake_activity
                .debit(stake_deposit);
            if success {
                staking_pool_balances.staked.credit(stake_deposit);
            }
            account.set_stake_balances(&staking_pool_id, &staking_pool_balances);
            self.accounts
                .upsert(&env::predecessor_account_id(), &account);
        }

        // track at the staking pool level
        {
            let mut activity = self.deposit_and_stake_activity.get(staking_pool_id).expect(
                format!(
                    "deposit_and_stake_activity does not exist for staking pool: {}",
                    staking_pool_id
                )
                .as_str(),
            );
            activity.debit(stake_deposit);
            self.deposit_and_stake_activity
                .insert(staking_pool_id, &activity);
        }
    }
}
