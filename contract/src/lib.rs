// TODO: remove
#![allow(unused_imports, dead_code, unused_variables)]

pub mod config;
pub mod contract;
pub mod core;
pub mod domain;
pub mod interface;
pub mod near;

pub use contract::*;

#[cfg(test)]
pub(crate) mod test_utils;

use crate::{
    config::Config,
    core::Hash,
    domain::{
        Account, BatchId, BlockHeight, RedeemStakeBatch, RedeemStakeBatchReceipt, StakeBatch,
        StakeBatchReceipt, StakeTokenValue, StorageUsage, TimestampedNearBalance,
        TimestampedStakeBalance, YoctoNear, YoctoNearValue, YoctoStake,
    },
    near::storage_keys::{
        ACCOUNTS_KEY_PREFIX, REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX,
        STAKE_BATCH_RECEIPTS_KEY_PREFIX,
    },
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, UnorderedMap},
    env,
    json_types::ValidAccountId,
    near_bindgen,
    serde::{Deserialize, Serialize},
    wee_alloc, AccountId,
};
use std::{
    convert::{TryFrom, TryInto},
    fmt::{self, Display, Formatter},
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakeTokenContract {
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

    /// the total amount of account storage fees that have been deposited and escrowed
    /// - when an account unregisters, it is refunded its storage fee deposit
    total_storage_escrow: TimestampedNearBalance,

    /// total NEAR balance across all accounts that is available for withdrawal
    /// - credits are applied when [RedeemStakeBatchReceipt] is created
    /// - debits are applied when account withdraws funds
    total_near: TimestampedNearBalance,
    /// total STAKE token supply in circulation
    /// - credits are applied when [StakeBatchReceipt] is created
    /// - debits are applied when account [RedeemStakeBatchReceipt] is created
    total_stake: TimestampedStakeBalance,

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
    /// unstaked NEAR funds are not available for 4 epochs after the funds were unstaked
    /// - [RedeemStakeBatch] can only be processed after all available unstaked NEAR funds have been
    ///   withdrawn, i.e., ig [pending_withdrawal] is None
    pending_withdrawal: Option<RedeemStakeBatchReceipt>,

    /// receipts serve 2 purposes:
    /// 1. receipts record batch results
    /// 2. receipts are used by account to claim funds
    ///    - once all funds are claimed from a receipt by accounts, then the receipt will be deleted
    ///      from storage
    ///    - if batches completed successfully, then accounts claim STAKE tokens
    ///    - if the batches failed. then receipt is never created - the batch can be retried
    stake_batch_receipts: UnorderedMap<BatchId, StakeBatchReceipt>,
    /// - if batches completed successfully, then accounts claim NEAR tokens
    /// - if the batches failed. then the receipt is never created - the batch can be retried
    redeem_stake_batch_receipts: UnorderedMap<BatchId, RedeemStakeBatchReceipt>,

    staking_pool_id: AccountId,
    locked: bool,
}

impl Default for StakeTokenContract {
    fn default() -> Self {
        panic!("contract must be initialized before usage")
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ContractSettings {
    pub staking_pool_id: ValidAccountId,
    pub config: Option<Config>,
    pub operator_id: ValidAccountId,
}

impl ContractSettings {
    /// depends on NEAR runtime env
    pub fn new(
        staking_pool_id: AccountId,
        operator_id: AccountId,
        config: Option<Config>,
    ) -> Result<Self, InvalidContractSettings> {
        let settings = Self {
            staking_pool_id: staking_pool_id
                .try_into()
                .map_err(|_| InvalidContractSettings::InvalidStakingPoolId)?,
            config,
            operator_id: operator_id
                .try_into()
                .map_err(|_| InvalidContractSettings::InvalidOperatorId)?,
        };

        match settings.validate() {
            Some(err) => Err(err),
            None => Ok(settings),
        }
    }

    pub fn validate(&self) -> Option<InvalidContractSettings> {
        if env::current_account_id().as_str() == self.operator_id.as_ref().as_str() {
            Some(InvalidContractSettings::OperatorMustNotBeContract)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum InvalidContractSettings {
    InvalidStakingPoolId,
    InvalidOperatorId,
    OperatorMustNotBeContract,
}

impl Display for InvalidContractSettings {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            InvalidContractSettings::OperatorMustNotBeContract => {
                write!(f, "operator account ID must not be the contract account ID")
            }
            InvalidContractSettings::InvalidOperatorId => write!(f, "invalid operator account ID"),
            InvalidContractSettings::InvalidStakingPoolId => {
                write!(f, "invalid staking pool account ID")
            }
        }
    }
}

#[near_bindgen]
impl StakeTokenContract {
    /// ## Notes
    /// - when the contract is deployed it will measure account storage usage
    #[payable]
    #[init]
    pub fn new(settings: ContractSettings) -> Self {
        assert!(!env::state_exists(), "contract is already initialized");

        if let Some(err) = settings.validate() {
            panic!(err.to_string());
        }

        let mut contract = Self {
            operator_id: settings.operator_id.into(),

            config: settings.config.unwrap_or_else(Config::default),
            config_change_block_height: env::block_index().into(),

            accounts: LookupMap::new(ACCOUNTS_KEY_PREFIX.to_vec()),
            accounts_len: 0,
            total_storage_escrow: Default::default(),
            total_near: Default::default(),
            total_stake: Default::default(),
            stake_token_value: Default::default(),
            batch_id_sequence: BatchId::default(),
            stake_batch: None,
            redeem_stake_batch: None,
            next_stake_batch: None,
            next_redeem_stake_batch: None,
            pending_withdrawal: None,
            stake_batch_receipts: UnorderedMap::new(STAKE_BATCH_RECEIPTS_KEY_PREFIX.to_vec()),
            redeem_stake_batch_receipts: UnorderedMap::new(
                REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX.to_vec(),
            ),
            account_storage_usage: Default::default(),
            staking_pool_id: settings.staking_pool_id.into(),
            locked: false,
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
        self.stake_batch_receipts
            .insert(&batch_id, &StakeBatchReceipt::default());
        self.redeem_stake_batch_receipts
            .insert(&batch_id, &RedeemStakeBatchReceipt::default());
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
    use crate::{interface::AccountManagement, near::YOCTO, test_utils::*};
    use near_sdk::{serde_json, testing_env, AccountId, MockedBlockchain, VMContext};
    use std::convert::TryFrom;

    #[test]
    fn contract_settings_serde_json() {
        testing_env!(new_context("bob.near"));

        let contract_settings = ContractSettings::new(
            "staking-pool.near".into(),
            "operator.stake.oysterpack.near".into(),
            Some(Config::default()),
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&contract_settings).unwrap();
        println!("{}", json);

        let contract_settings: ContractSettings = serde_json::from_str(
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
        let contract = StakeTokenContract::new(contract_settings.clone());

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
        assert!(!contract.locked, "contract should not be locked");
        assert_eq!(
            contract.total_storage_escrow.balance().value(),
            0,
            "the total storage escrow should be zero"
        );
        assert_eq!(
            contract.total_near.balance().value(),
            0,
            "the total NEAR balance aggregated across all account should be zero"
        );
        assert_eq!(
            contract.total_stake.balance().value(),
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
        // And there should be no receipts
        assert!(contract.stake_batch_receipts.is_empty());
        assert!(contract.redeem_stake_batch_receipts.is_empty())
    }
}
