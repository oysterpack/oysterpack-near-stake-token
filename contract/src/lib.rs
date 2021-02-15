//! # OysterPack STAKE Token NEAR Smart Contract
//! > With the OysterPack NEAR STAKE token "You can have your STAKE and TRADE it too"
//!
//! The OysterPack STAKE token is backed by staked NEAR. This contract enables you to delegate your
//! NEAR to stake, and in return you are issued STAKE tokens. This enables you to trade your STAKE
//! tokens while your NEAR is staked and earning staking rewards. The STAKE token transforms your
//! staked NEAR into a tradeable asset.
//!
//! STAKE token value is pegged to NEAR token value and stake earnings. As staking rewards are earned,
//! the STAKE token value increases. In other words, STAKE tokens appreciate in NEAR token value over
//! time.
//!
//! In addition, the contract provides the following yield boosting levers:
//! 1. the contract owner can share a percentage of the contract's gas rewards with STAKE user accounts
//!    to boost yield. When funds are staked, contract gas earnings will be distributed to STAKE users
//!    by staking the NEAR funds into the staking pool, which increases the staked NEAR balance, which
//!    increases the STAKE token value.
//!
//! 2. the contract supports collecting earnings from other contracts into the STAKE token contract.
//!    The collected earnings are pooled with the STAKE Token contract gas earnings and distributed
//!    to the contract owner and user accounts.
//!
//! When redeeming STAKE tokens for NEAR, the STAKE token contract also helps to add liquidity for
//! withdrawing your unstaked NEAR tokens (see below for more details)
//!
//! # STAKE Token Vision
//! > harness the Internet of value - everything on the internet can take on the properties of money
//!
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
//! staking while unstaked near is locked up in the staking pool awaiting to become available to
//! be withdrawn.
//!
//! ## STAKE Token Benefits
//! 1. NEAR token asset value is maximized through staking and gas earnings profit sharing. The more
//!    the contract and token is used, the more it's worth.
//! 2. Transforms staked NEAR into tradeable digital asset, i.e., into a fungible token.
//! 3. Provides more incentive to stake NEAR, which helps to further strengthen and secure the network
//!    by providing more economic incentive to validators.
//! 4. Can be used to collect income streams from contracts - transforming contracts into income
//!    producing assets.
//!
//! # Contract Key Features and High Level Design
//! - Contract users must register with the account in order to use it. Users must pay an upfront
//!   account storage usage fee because long term storage is not "free" on NEAR. When an account
//!   unregisters, the storage usage fee will be refunded.
//! - STAKE token contract is linked to a single staking pool contract that is specified as part of
//!   contract deployment and becomes permanent for contract's lifetime. A STAKE token contract will
//!   be deployed per staking pool contract.
//! - STAKE token is a fungible token and supports multiple transfer protocols:
//!   - simple token transfer between accounts - modeled after [NEP-21 Fungible Token](https://nomicon.io/Standards/Tokens/FungibleToken.html)
//!   - more advanced token transfers between contracts:
//!     - vault based token transfer modeled afer [NEP-122 vault based fungible token standard](https://github.com/near/NEPs/issues/122)
//!     - transfer and notifiy modeled after [NEP-136 interactive Fungible Token](https://github.com/near/NEPs/issues/122) and
//!       [NEP-110 Advanced Fungible Token Standard](https://github.com/near/NEPs/issues/110)
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
//! - [FungibleTokenCore](crate::interface::FungibleToken)
//! - [Operator](crate::interface::Operator)
//! - [ContractOwner](crate::interface::ContractOwner)
//! - [ContractFinancials](crate::interface::ContractFinancials)
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

pub mod config;
mod contract;
pub mod core;
pub mod domain;
pub mod errors;
pub mod interface;
pub mod near;

pub(crate) use contract::*;

#[cfg(test)]
pub(crate) mod test_utils;

use crate::domain::StakeLock;
use crate::{
    config::Config,
    core::Hash,
    domain::{
        Account, BatchId, BlockHeight, RedeemLock, RedeemStakeBatch, RedeemStakeBatchReceipt,
        StakeBatch, StakeBatchReceipt, StakeTokenValue, StorageUsage, TimestampedNearBalance,
        TimestampedStakeBalance, YoctoNear,
    },
    near::storage_keys::{
        ACCOUNTS_KEY_PREFIX, REDEEM_STAKE_BATCH_RECEIPTS_KEY_PREFIX,
        STAKE_BATCH_RECEIPTS_KEY_PREFIX,
    },
};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::LookupMap,
    env,
    json_types::ValidAccountId,
    near_bindgen, wee_alloc, AccountId, PanicOnDefault,
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    /// contract owner
    owner_id: AccountId,

    /// contract owner balance pays for contract storage separate from user account storage fees
    /// - this means part of the contract owner balance is always locked to cover `contract_initial_storage_usage`
    contract_owner_balance: YoctoNear,
    /// initial contract storage usage is recorded to track the amount of storage that the contract
    /// owner is responsible to pay for. In addition, it is useful to track and monitor storage usage
    /// growth.
    /// - storage is the most important resource to manage and track on NEAR
    contract_initial_storage_usage: StorageUsage,
    /// the contract is designed to collect deposits which will be staked to boost STAKE value for user accounts
    collected_earnings: YoctoNear,

    /// Operator is allowed to perform operator actions on the contract
    operator_id: AccountId,

    config: Config,
    /// when the config was last changed
    /// the block info can be looked up via its block index: https://docs.near.org/docs/api/rpc#block
    config_change_block_height: BlockHeight,

    /// how much storage the account needs to pay for when registering an account
    /// - dynamically computed when the contract is deployed
    account_storage_usage: StorageUsage,
    /// we need to track the storage escrow balance because we can't assume storage staking cost will
    /// remain constant on NEAR
    total_account_storage_escrow: YoctoNear,

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
    /// - when the current `stake_batch` has completed processing, then this batch is "promoted"
    ///   to the current `stake_batch`
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
    stake_batch_lock: Option<StakeLock>,
    redeem_stake_batch_lock: Option<RedeemLock>,

    #[cfg(test)]
    #[borsh_skip]
    env: near_env::Env,
}

#[near_bindgen]
impl Contract {
    /// ## Notes
    /// - when the contract is deployed it will measure account storage usage
    /// - owner account ID defaults to the operator account ID
    #[init]
    pub fn new(
        staking_pool_id: ValidAccountId,
        owner_id: ValidAccountId,
        operator_id: ValidAccountId,
    ) -> Self {
        assert!(!env::state_exists(), "contract is already initialized");
        assert_ne!(env::current_account_id().as_str(), owner_id.as_ref());
        assert_ne!(env::current_account_id().as_str(), operator_id.as_ref());

        let mut contract = Self {
            owner_id: owner_id.into(),
            contract_owner_balance: env::account_balance().into(),

            operator_id: operator_id.into(),

            config: Config::default(),
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
            staking_pool_id: staking_pool_id.into(),
            stake_batch_lock: None,
            redeem_stake_batch_lock: None,

            total_account_storage_escrow: 0.into(),
            contract_initial_storage_usage: 0.into(), // computed after contract is created - see below
            collected_earnings: 0.into(),

            #[cfg(test)]
            env: near_env::Env::default(),
        };

        // compute initial_contract_storage_usage
        // the contract state is not yet saved to storage - measure it's storage usage manually by
        // serializing its state via borsh. In addition to the serialized bytes, there is some storage
        // overhead - which was determined to be 45 from sim tests
        let state_storage_overhead = 45;
        contract.contract_initial_storage_usage = (env::storage_usage()
            + contract.try_to_vec().unwrap().len() as u64
            + state_storage_overhead)
            .into();

        // compute account storage usage
        {
            let initial_storage_usage = env::storage_usage();
            contract.allocate_account_template_to_measure_storage_usage();
            contract.account_storage_usage =
                StorageUsage(env::storage_usage() - initial_storage_usage);
            contract.deallocate_account_template_to_measure_storage_usage();
            assert_eq!(initial_storage_usage, env::storage_usage());
        }

        // for testing purposes, inject a successful PromiseResult
        // - this enables callbacks that have callback data dependencies to be unit tested because
        //   the callbacks check if the promise call succeeded. Without this, the callbacks would
        //   not be able to be unit tested because the NEAR VMContext does not provide ability to
        //   inject receipts.
        #[cfg(test)]
        {
            crate::test_utils::set_env_with_success_promise_result(&mut contract);
        }

        contract
    }
}

impl Contract {
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
    use crate::{interface::AccountManagement, test_utils::*};
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn default_constructor_should_fail() {
        let account_id = "bob.near";
        let mut context = new_context(account_id);
        context.block_index = 10;
        testing_env!(context);

        Contract::default();
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
        // Arrange
        let test_ctx = TestContext::new();

        // Assert
        pub const EXPECTED_ACCOUNT_STORAGE_USAGE: u64 = 681;
        assert_eq!(
            test_ctx.account_storage_usage.value(),
            EXPECTED_ACCOUNT_STORAGE_USAGE
        );
        assert_eq!(
            test_ctx.account_storage_fee().value(),
            EXPECTED_ACCOUNT_STORAGE_USAGE as u128
                * test_ctx.config.storage_cost_per_byte().value()
        );

        assert_eq!(
            test_ctx.total_registered_accounts().0,
            0,
            "there should be no accounts registered"
        );
        assert_eq!(
            test_ctx.config_change_block_height.value(),
            0,
            "config change block height should be set from the NEAR runtime env"
        );
        assert!(
            !test_ctx.stake_batch_locked(),
            "contract should not be locked"
        );
        assert_eq!(
            test_ctx.total_near.amount().value(),
            0,
            "the total NEAR balance aggregated across all account should be zero"
        );
        assert_eq!(
            test_ctx.total_stake.amount().value(),
            0,
            "the total STAKE supply should be zero"
        );
        assert_eq!(
            test_ctx.batch_id_sequence.value(),
            0,
            "batch ID sequence should be zero"
        );
        // And batches should be None
        assert!(test_ctx.stake_batch.is_none());
        assert!(test_ctx.redeem_stake_batch.is_none());

        assert_eq!(test_ctx.owner_id, TEST_OWNER_ID);
        assert_eq!(test_ctx.operator_id, TEST_OPERATOR_ID);

        println!(
            "initial_contract_storage_usage = {:?}",
            test_ctx.contract_initial_storage_usage
        );
    }
}
