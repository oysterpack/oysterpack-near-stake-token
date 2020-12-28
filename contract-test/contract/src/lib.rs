#![allow(dead_code, unused_variables, unused_imports)]

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{ValidAccountId, U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, PromiseOrValue};
use near_sdk::{wee_alloc, AccountId, Promise, PromiseResult};
use std::convert::TryFrom;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(Default, BorshDeserialize, BorshSerialize)]
pub struct TestHarness {}

#[near_bindgen]
impl TestHarness {
    pub fn ping() -> String {
        format!(
            "block timestamp: {}, epoch: {}, block index: {} ",
            env::block_timestamp(),
            env::epoch_height(),
            env::block_index()
        )
    }

    /// - register the account if not already registered
    /// - lookup the account info and return it
    pub fn test_account_registration_workflow(
        &self,
        stake_token_contract: ValidAccountId,
    ) -> Promise {
        log(format!(
            "STAKE Token Contract: {}",
            stake_token_contract.as_ref()
        ));

        let callback_gas = env::prepaid_gas().saturating_sub(TGAS * 20);
        // log(format!("callback_gas = {}", callback_gas));

        account_management::account_registered(
            to_valid_account(env::current_account_id()),
            &to_account(&stake_token_contract),
            NO_DEPOSIT,
            TGAS * 5,
        )
        .then(ext_self::on_account_registered(
            to_account(&stake_token_contract),
            &env::current_account_id(),
            NO_DEPOSIT,
            TGAS * 250,
        ))
    }
}

/// test_account_registration_workflow
#[near_bindgen]
impl TestHarness {
    /// if registered, then unregister the account and restart test workflow
    /// if not registered, then register the contract account and return the contract StakeAccount info
    pub fn on_account_registered(
        &self,
        stake_token_contract: AccountId,
        #[callback] registered: bool,
    ) -> Promise {
        assert_predecessor_is_self();

        assert!(
            promise_result_succeeded(),
            "StakeTokenContract::on_account_registered failed: {}",
            stake_token_contract
        );

        if registered {
            log(format!(
                "account is already registered: {}",
                env::current_account_id()
            ));
            account_management::lookup_account(
                to_valid_account(env::current_account_id()),
                &stake_token_contract,
                NO_DEPOSIT,
                TGAS * 5,
            )
        } else {
            log(format!(
                "account is not registered: {}",
                env::current_account_id()
            ));

            let callback_gas = env::prepaid_gas().saturating_sub(TGAS * 20);
            log(format!("callback_gas = {}", callback_gas));

            account_management::account_storage_fee(&stake_token_contract, NO_DEPOSIT, TGAS * 5)
                .then(ext_self::on_account_storage_fee_self_register(
                    stake_token_contract,
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    TGAS * 100,
                ))
        }
    }

    /// registers the account with the account storage fee obtained from the STAKE Token contract
    pub fn on_account_storage_fee_self_register(
        &self,
        stake_token_contract: AccountId,
        #[callback] storage_fee: YoctoNear,
    ) -> Promise {
        assert_predecessor_is_self();

        assert!(
            promise_result_succeeded(),
            "StakeTokenContract::account_storage_fee failed: {}",
            stake_token_contract
        );

        log(format!(
            "account storage fee is {} yoctoNEAR",
            storage_fee.0 .0
        ));

        let callback_gas = env::prepaid_gas().saturating_sub(TGAS * 20);
        // log(format!("callback_gas = {}", callback_gas));

        account_management::register_account(&stake_token_contract, storage_fee.0 .0, TGAS * 5)
            .then(ext_self::on_register_account_lookup_account(
                stake_token_contract,
                &env::current_account_id(),
                NO_DEPOSIT,
                TGAS * 40,
            ))
    }

    pub fn on_register_account_lookup_account(&self, stake_token_contract: AccountId) -> Promise {
        assert_predecessor_is_self();

        assert!(
            promise_result_succeeded(),
            "StakeTokenContract::register_account failed: {}",
            stake_token_contract
        );

        log(format!(
            "successfully registered with: {}",
            stake_token_contract
        ));

        account_management::lookup_account(
            to_valid_account(env::current_account_id()),
            &stake_token_contract,
            NO_DEPOSIT,
            TGAS * 5,
        )
    }
}

#[ext_contract(ext_self)]
pub trait ExtSelf {
    fn on_account_registered(
        &self,
        stake_token_contract: AccountId,
        #[callback] registered: bool,
    ) -> Promise;

    fn on_account_storage_fee_self_register(
        &self,
        stake_token_contract: AccountId,
        #[callback] storage_fee: YoctoNear,
    ) -> Promise;

    fn on_register_account_lookup_account(&self, stake_token_contract: AccountId) -> Promise;
}

#[ext_contract(account_management)]
pub trait AccountManagement {
    /// Creates and registers a new account for the predecessor account ID.
    /// - the account is required to pay for its storage. Storage fees will be escrowed and then refunded
    ///   when the account is unregistered - use [account_storage_escrow_fee](crate::interface::AccountManagement::account_storage_fee)
    ///   to lookup the required storage fee amount. Overpayment of storage fee is refunded.
    ///
    /// ## Panics
    /// - if deposit is not enough to cover storage usage fees
    /// - if account is already registered
    fn register_account(&mut self);

    /// In order to unregister the account all NEAR must be unstaked and withdrawn from the account.
    /// The escrowed storage fee will be refunded to the account.
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if registered account has funds
    fn unregister_account(&mut self);

    /// Returns the required deposit amount that is required for account registration.
    fn account_storage_fee(&self) -> YoctoNear;

    /// returns true if the account is registered
    fn account_registered(&self, account_id: ValidAccountId) -> bool;

    /// returns the total number of accounts that are registered with this contract
    fn total_registered_accounts(&self) -> U128;

    /// looks up the registered account
    fn lookup_account(&self, account_id: ValidAccountId) -> Option<StakeAccount>;

    /// Withdraws the specified amount from the account's available NEAR balance and transfers the
    /// funds to the account.
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are not enough available NEAR funds to fulfill the request
    fn withdraw(&mut self, amount: YoctoNear);

    /// Withdraws all available NEAR funds from the account and transfers the
    /// funds to the account.
    ///
    /// ## Panics
    /// - if the account is not registered
    /// - if there are no funds to withdraw
    fn withdraw_all(&mut self);
}

/// View model for a registered account with the contract
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeAccount {
    /// account storage usage payment that is escrowed
    /// - the balance will be refunded when the account unregisters
    /// - timestamp also shows when the account registered
    pub storage_escrow: TimestampedNearBalance,

    /// NEAR balance that is available for withdrawal from the contract
    pub near: Option<TimestampedNearBalance>,
    /// account STAKE token balance
    pub stake: Option<TimestampedStakeBalance>,

    /// NEAR funds that have been deposited to be staked when the batch is run
    pub stake_batch: Option<StakeBatch>,
    /// While batches are running, the contract is locked. The account can still deposit NEAR funds
    /// to stake into the next batch while the contract is locked.
    pub next_stake_batch: Option<StakeBatch>,

    /// STAKE tokens that have been set aside to be redeemed in the next batch
    pub redeem_stake_batch: Option<RedeemStakeBatch>,
    /// While batches are running, the contract is locked. The account can still set submit requests
    /// to redeem STAKE tokens into the next batch while the contract is locked.
    pub next_redeem_stake_batch: Option<RedeemStakeBatch>,
}

pub const NO_DEPOSIT: u128 = 0;

/// 1 teraGas
pub const TGAS: u64 = 1_000_000_000_000;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct YoctoNear(pub U128);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct YoctoStake(pub U128);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TimestampedNearBalance {
    pub amount: YoctoNear,
    pub block_height: BlockHeight,
    pub block_timestamp: BlockTimestamp,
    pub epoch_height: EpochHeight,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TimestampedStakeBalance {
    pub amount: YoctoStake,
    pub block_height: BlockHeight,
    pub block_timestamp: BlockTimestamp,
    pub epoch_height: EpochHeight,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct BlockHeight(pub U64);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct BlockTimestamp(pub U64);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct EpochHeight(pub U64);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct BlockTimeHeight {
    pub block_height: BlockHeight,
    pub block_timestamp: BlockTimestamp,
    pub epoch_height: EpochHeight,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeBatch {
    pub id: BatchId,
    pub balance: TimestampedNearBalance,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct BatchId(pub U128);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RedeemStakeBatch {
    pub id: BatchId,
    pub balance: TimestampedStakeBalance,
    /// if receipt is present it means the STAKE has been redeemed and the unstaked NEAR is still locked
    /// by the staking pool for withdrawal
    pub receipt: Option<RedeemStakeBatchReceipt>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RedeemStakeBatchReceipt {
    /// tracks amount of STAKE that has been claimed on the receipt
    /// - when the amount reaches zero, then the receipt is deleted
    pub redeemed_stake: YoctoStake,

    /// the STAKE token value at the point in time when the batch was run
    /// - is used to compute the amount of STAKE tokens to issue to the account based on the amount
    ///   of NEAR that was staked
    pub stake_token_value: StakeTokenValue,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeTokenValue {
    pub block_time_height: BlockTimeHeight,
    pub total_staked_near_balance: YoctoNear,
    pub total_stake_supply: YoctoStake,
}

fn to_account(account: &ValidAccountId) -> AccountId {
    account.as_ref().to_string()
}

fn to_valid_account(account_id: AccountId) -> ValidAccountId {
    ValidAccountId::try_from(account_id).unwrap()
}

/// asserts that predecessor account is the contract itself - used to enforce that callbacks
/// should only be called internally - even though they are exposed on the public contract interface
fn assert_predecessor_is_self() {
    if env::predecessor_account_id() != env::current_account_id() {
        panic!("function can only be called by self")
    }
}

/// wrapper around `near_sdk::env::log()` to make it simpler to use
fn log(msg: String) {
    env::log(msg.as_bytes());
}

fn promise_result_succeeded() -> bool {
    match env::promise_result(0) {
        PromiseResult::Successful(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn quick_test() {}
}
