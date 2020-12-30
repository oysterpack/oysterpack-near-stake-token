//! # OysterPack STAKE Token NEAR Smart Contract
//! The OysterPack STAKE token is backed by staked NEAR. This contract enables you to delegate your
//! NEAR to stake, and in return you are issued STAKE tokens. This enables you to trade your STAKE
//! tokens while your NEAR is staked and earning staking rewards. The STAKE token transforms your
//! staked NEAR into a tradeable asset.
//!
//! STAKE token value is pegged to NEAR token value and stake earnings. As staking rewards are earned,
//! the STAKE token value increases. In other words, STAKE tokens appreciate in NEAR token value over
//! time.
//!
//! ## STAKE Token Vision
//! Leverage NEAR as a digital currency beyond being a utility token for the NEAR network to pay for
//! transaction gas and storage usage. NEAR is designed to be scalable and fast with very low and
//! predictable transaction costs and pricing. NEAR tokenomics has built in inflation, with a 5%
//! maximum inflation target. The inflation provides incentive to stake your NEAR, which helps to further
//! secure the network. Transforming staked NEAR into a tradeable asset via the STAKE token enhances
//! the value proposition. Since most of the NEAR token supply will be staked, we can get more value
//! out of the staked NEAR by being able to use it as a tradeable digital asset.
//!
//! The long term vision is to integrate the STAKE token with the NEAR wallet:
//! - users would be able to stake their NEAR via this contract
//! - users would be able to transfer STAKE tokens via the NEAR wallet
//!
//! ## Problem With Current Unstaking Process
//! With the current staking pool implementations, the problem is that unstaked NEAR is not immediately
//! available for withdrawal from the staking pool contract. The unstaked NEAR is locked for 4 epoch
//! time periods, which translates to ~48 hours in NEAR time. This makes it more difficult and complex
//! to utilize NEAR as a digital asset, i.e., as a fungible token. The OyserPack STAKE Token Contract
//! helps to bypass the lockup period by providing liquidity.
//!
//! ### How STAKE Token Contract Provides Liquidity To Bypass Staking Pool Contract NEAR Lockup
//! When the contract runs a [StakeBatch](crate::domain::StakeBatch), the contract will check if
//! there is a pending withdrawal for unstaked NEAR. The contract will then apply the unstaked NEAR
//! to the transaction, and move NEAR funds from the [StakeBatch](crate::domain::StakeBatch) into the
//! contract's NEAR liquidity pool. This makes the unstaked NEAR "liquid" and available for withdrawal
//! immediately without waiting for the unstaked NEAR to become available for withdrawal from the staking
//! pool. Users will be able to withdraw from the contract's NEAR liquidity pool on a first come first
//! serve basis. The amount of liquidity is determined by contract activity, i.e., how much users are
//! staking while there unstaked near is locked up in the staking pool awaiting to become available to
//! be withdrawn.
//!
//! ## STAKE Token Benefits
//! 1. NEAR token asset value is maximized through staking.
//! 2. Transforms staked NEAR into tradeable digital asset, i.e., into a fungible token.
//! 3. Provides more incentive to stake NEAR, which helps to further strengthen and secure the network
//!    by providing more economic incentive to validators.
//!
//! # Contract Key Features and High Level Design
//! - Contract users must register with the account in order to use it. Users must pay an upfront
//!   account storage usage fee because long term storage is not "free" on NEAR. When an account
//!   unregisters, the storage usage fee will be refunded.
//! - STAKE token contract is linked to a single staking pool contract that is specified as part of
//!   contract deployment and becomes permanent for contract's lifetime. A STAKE token contract will
//!   be deployed per staking pool contract.
//! - Implements [NEP-122 vault based fungible token standard](https://github.com/near/NEPs/issues/122)
//!   - NEAR community is currently trying to standardize fungible token interface. STAKE token implements
//!     NEP-122 Vault Based Fungible Token (WIP), but waiting for NEP-122 standard to be finalized.
//! - Has concept of contract ownership. The contract owner earns the contract rewards from transaction
//!   fees.
//!   - contract ownership can be transferred
//!   - contract earning can be staked into the contract owner's account
//! - Contract has an operator role which provides functions to support the contract, e.g., releasing
//!   locks, config management, etc
//!
//! # STAKE Token Contract Design
//! The STAKE token contract [interfaces](crate::interface) are defined as traits:
//! - [AccountManagement](crate::interface::AccountManagement)
//! - [StakingService](crate::interface::StakingService)
//! - [FungibleToken](crate::interface::FungibleToken)
//!   - supports following token transfer protocols:
//!     - [simple](crate::interface::SimpleTransfer)
//!     - [vault based](crate::interface::VaultBasedTransfer)
//!     - [transfer-call](crate::interface::TransferCall)
//! - [Operator](crate::interface::Operator)
//! - [ContractOwner](crate::interface::ContractOwner)
//!
//! See each of the interfaces for details.
//!
//! Contract **view** and **change** functions follow Rust semantics, i.e., interface methods with an
//! immutable receiver are **view** functions, and interface methods with a mutable receiver are
//! **change** functions.
//!
//! ## How is the STAKE token value computed?
//! STAKE token value in NEAR = `total staked NEAR balance / total STAKE token supply`
//!

// TODO: comment out when doing a release build
// #![allow(dead_code, unused_variables)]

pub mod config;
mod contract;
pub mod core;
pub mod domain;
pub mod errors;
pub mod interface;
pub mod near;

pub use contract::settings::*;
pub(crate) use contract::*;
pub(crate) use errors::*;

#[cfg(test)]
pub(crate) mod test_utils;

use crate::domain::YoctoNear;
use crate::{
    config::Config,
    core::Hash,
    domain::{
        Account, BatchId, BlockHeight, RedeemLock, RedeemStakeBatch, RedeemStakeBatchReceipt,
        StakeBatch, StakeBatchReceipt, StakeTokenValue, StorageUsage, TimestampedNearBalance,
        TimestampedStakeBalance, Vault, VaultId,
    },
    near::storage_keys::{
        ACCOUNTS_KEY_PREFIX, REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX,
        STAKE_BATCH_RECEIPTS_KEY_PREFIX, VAULTS_KEY_PREFIX,
    },
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::LookupMap,
    env,
    json_types::ValidAccountId,
    near_bindgen, wee_alloc, AccountId,
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakeTokenContract {
    /// contract owner
    owner_id: AccountId,
    /// Operator is allowed to perform operator actions on the contract
    /// TODO: support multiple operator and role management
    operator_id: AccountId,

    config: Config,
    /// when the config was last changed
    /// the block info can be looked up via its block index: https://docs.near.org/docs/api/rpc#block
    config_change_block_height: BlockHeight,
    /// how much storage the account needs to pay for when registering an account
    /// - dynamically computed when the contract is deployed
    account_storage_usage: StorageUsage,

    accounts: LookupMap<Hash, Account>,
    accounts_len: u128,

    /// total NEAR balance across all accounts that is available for withdrawal
    /// - credits are applied when [RedeemStakeBatchReceipt] is created
    /// - debits are applied when account withdraws funds
    total_near: TimestampedNearBalance,
    /// total STAKE token supply in circulation
    /// - credits are applied when [StakeBatchReceipt] is created
    /// - debits are applied when [RedeemStakeBatchReceipt] is created
    total_stake: TimestampedStakeBalance,

    /// used to provide liquidity when accounts are redeeming stake
    /// - funds will be drawn from the liquidity pool to fulfill requests to redeem STAKE
    /// - when batch receipts are claimed, the liquidity pool will be checked if unstaked NEAR funds
    ///   are still locked up in the staking pool
    /// - liquidity is automatically added when accounts are staking - the NEAR deposits will be added
    ///   to the liquidity pool if there are unstaked funds in the staking pool - the unstaked funds
    ///   will simply be restaked
    near_liquidity_pool: YoctoNear,

    /// cached value - if the epoch has changed, then the STAKE token value is out of date because
    /// stake rewars are issued every epoch.
    stake_token_value: StakeTokenValue,

    /// used to generate new batch IDs
    /// - the sequence is incremented to generate a new batch ID
    /// - sequence ID starts at 1
    batch_id_sequence: BatchId,

    /// tracks how much NEAR the account is has deposited into the current batch to be staked
    /// - when the batch run completes, a [StakeBatchReceipt] is created and recorded
    stake_batch: Option<StakeBatch>,
    /// when the contract is locked, i.e., a batch is being run, then NEAR funds are deposited
    /// into the next batch to be staked
    /// - when the current [stake_batch] has completed processing, then this batch is "promoted"
    ///   to the current [stake_batch]
    next_stake_batch: Option<StakeBatch>,

    redeem_stake_batch: Option<RedeemStakeBatch>,
    /// used to store batch requests while the contract is locked    
    next_redeem_stake_batch: Option<RedeemStakeBatch>,

    /// receipts serve 2 purposes:
    /// 1. receipts record batch results
    /// 2. receipts are used by account to claim funds
    ///    - once all funds are claimed from a receipt by accounts, then the receipt will be deleted
    ///      from storage
    ///    - if batches completed successfully, then accounts claim STAKE tokens
    ///    - if the batches failed. then receipt is never created - the batch can be retried
    stake_batch_receipts: LookupMap<BatchId, StakeBatchReceipt>,
    /// - if batches completed successfully, then accounts claim NEAR tokens
    /// - if the batches failed. then the receipt is never created - the batch can be retried
    redeem_stake_batch_receipts: LookupMap<BatchId, RedeemStakeBatchReceipt>,

    staking_pool_id: AccountId,
    run_stake_batch_locked: bool,
    run_redeem_stake_batch_lock: Option<RedeemLock>,

    /// for NEP-122 - vault-based fungible token
    vaults: LookupMap<VaultId, Vault>,
    vault_id_sequence: VaultId,

    #[cfg(test)]
    #[borsh_skip]
    env: near_env::Env,
}

impl Default for StakeTokenContract {
    fn default() -> Self {
        panic!("contract must be initialized before usage")
    }
}

#[near_bindgen]
impl StakeTokenContract {
    /// ## Notes
    /// - when the contract is deployed it will measure account storage usage
    /// - owner account ID defaults to the operator account ID
    ///
    /// TODO: verify the staking pool - contract is disabled until staking pool is verified via transation
    ///       If the staking pool contract fails verification, then the operator can delete the this contract.
    ///       NOTE: verification may fail if the contract mis-configured
    ///
    #[payable]
    #[init]
    pub fn new(owner_id: Option<ValidAccountId>, settings: ContractSettings) -> Self {
        assert!(!env::state_exists(), "contract is already initialized");

        settings.validate();

        let operator_id: AccountId = settings.operator_id.into();
        let mut contract = Self {
            owner_id: owner_id.map_or(operator_id.clone(), |account_id| account_id.into()),
            operator_id: operator_id,

            config: settings.config.unwrap_or_else(Config::default),
            config_change_block_height: env::block_index().into(),

            accounts: LookupMap::new(ACCOUNTS_KEY_PREFIX.to_vec()),
            accounts_len: 0,
            total_near: TimestampedNearBalance::new(0.into()),
            total_stake: TimestampedStakeBalance::new(0.into()),
            near_liquidity_pool: 0.into(),
            stake_token_value: StakeTokenValue::default(),
            batch_id_sequence: BatchId::default(),
            stake_batch: None,
            redeem_stake_batch: None,
            next_stake_batch: None,
            next_redeem_stake_batch: None,
            stake_batch_receipts: LookupMap::new(STAKE_BATCH_RECEIPTS_KEY_PREFIX.to_vec()),
            redeem_stake_batch_receipts: LookupMap::new(
                REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX.to_vec(),
            ),
            account_storage_usage: Default::default(),
            staking_pool_id: settings.staking_pool_id.into(),
            run_stake_batch_locked: false,
            run_redeem_stake_batch_lock: None,

            vaults: LookupMap::new(VAULTS_KEY_PREFIX.to_vec()),
            vault_id_sequence: VaultId::default(),
            #[cfg(test)]
            env: near_env::Env::default(),
        };

        // compute account storage usage
        {
            let initial_storage_usage = env::storage_usage();
            contract.allocate_account_template_to_measure_storage_usage();
            contract.account_storage_usage =
                StorageUsage(env::storage_usage() - initial_storage_usage);
            contract.deallocate_account_template_to_measure_storage_usage();
            assert_eq!(initial_storage_usage, env::storage_usage());
        }

        #[cfg(test)]
        {
            pub fn promise_result(_result_index: u64) -> near_sdk::PromiseResult {
                near_sdk::PromiseResult::Successful(vec![])
            }

            pub fn promise_results_count() -> u64 {
                1
            }

            contract.set_env(near_env::Env {
                promise_results_count_: promise_results_count,
                promise_result_: promise_result,
            });
        }

        contract
    }
}

impl StakeTokenContract {
    /// this is used to compute the storage usage fees to charge for account registration
    /// - the account is responsible to pay for its storage fees - account storage is allocated, measured,
    ///   and then freed
    fn allocate_account_template_to_measure_storage_usage(&mut self) {
        let hash = Hash::from([0u8; 32]);
        let account_template = Account::account_template_to_measure_storage_usage();
        self.accounts.insert(&hash, &account_template);

        let batch_id = BatchId(0);
        self.stake_batch_receipts.insert(
            &batch_id,
            &StakeBatchReceipt::new(0.into(), StakeTokenValue::default()),
        );
        self.redeem_stake_batch_receipts.insert(
            &batch_id,
            &RedeemStakeBatchReceipt::new(0.into(), StakeTokenValue::default()),
        );
    }

    fn deallocate_account_template_to_measure_storage_usage(&mut self) {
        let hash = Hash::from([0u8; 32]);
        self.accounts.remove(&hash);

        let batch_id = BatchId(0);
        self.stake_batch_receipts.remove(&batch_id);
        self.redeem_stake_batch_receipts.remove(&batch_id);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::interface::StakingService;
    use crate::{interface::AccountManagement, test_utils::*};
    use near_sdk::{serde_json, testing_env, MockedBlockchain};

    #[test]
    fn contract_settings_serde_json() {
        testing_env!(new_context("bob.near"));

        let contract_settings = ContractSettings::new(
            "staking-pool.near".into(),
            "operator.stake.oysterpack.near".into(),
            Some(Config::default()),
        );
        let json = serde_json::to_string_pretty(&contract_settings).unwrap();
        println!("{}", json);

        let _contract_settings: ContractSettings = serde_json::from_str(
            r#"{
  "staking_pool_id": "staking-pool.near",
  "operator_id": "operator.stake.oysterpack.near"
}"#,
        )
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "contract must be initialized before usage")]
    fn default_constructor_should_fail() {
        let account_id = "bob.near";
        let mut context = new_context(account_id);
        context.block_index = 10;
        testing_env!(context);

        StakeTokenContract::default();
    }

    /// When the contract is deployed
    /// Then [StakeTokenContract::account_storage_usage] is dynamically computed
    /// And staking pool ID was stored
    /// And there should be no accounts registered
    /// And the config change block height should be set from the NEAR runtime env
    /// And the contract should not be locked
    /// And the total storage escrow should be zero
    /// And the total NEAR balance aggregated across all account should be zero
    /// And the total STAKE supply should be zero
    /// And batch ID sequence should be zero
    /// And batches should be None
    /// And there should be no receipts
    #[test]
    fn contract_init() {
        let account_id = "bob.near";
        let mut context = new_context(account_id);
        context.block_index = 10;
        testing_env!(context);

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(None, contract_settings.clone());

        assert_eq!(
            &contract.staking_pool_id(),
            contract_settings.staking_pool_id.as_ref()
        );

        // Then [StakeTokenContract::account_storage_usage] is dynamically computed
        assert_eq!(
            contract.account_storage_usage.value(),
            EXPECTED_ACCOUNT_STORAGE_USAGE
        );
        assert_eq!(
            contract.account_storage_fee().value(),
            EXPECTED_ACCOUNT_STORAGE_USAGE as u128
                * contract.config.storage_cost_per_byte().value()
        );

        // And staking pool ID was stored
        assert_eq!(
            contract.staking_pool_id.as_str(),
            contract_settings.staking_pool_id.as_ref().as_str()
        );

        assert_eq!(
            contract.total_registered_accounts().0,
            0,
            "there should be no accounts registered"
        );
        assert_eq!(
            contract.config_change_block_height.value(),
            10,
            "config change block height should be set from the NEAR runtime env"
        );
        assert!(
            !contract.run_stake_batch_locked,
            "contract should not be locked"
        );
        assert_eq!(
            contract.total_near.amount().value(),
            0,
            "the total NEAR balance aggregated across all account should be zero"
        );
        assert_eq!(
            contract.total_stake.amount().value(),
            0,
            "the total STAKE supply should be zero"
        );
        assert_eq!(
            contract.batch_id_sequence.value(),
            0,
            "batch ID sequence should be zero"
        );
        // And batches should be None
        assert!(contract.stake_batch.is_none());
        assert!(contract.redeem_stake_batch.is_none());

        assert_eq!(contract.owner_id, contract.operator_id);
    }
}
