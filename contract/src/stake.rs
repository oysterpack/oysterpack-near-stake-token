use crate::common::{YoctoNEAR, YoctoSTAKE};
use crate::state;
use near_sdk::{
    json_types::{ValidAccountId, U128, U64},
    serde::{Deserialize, Serialize},
    AccountId, Balance, BlockHeight, EpochHeight, Promise, PromiseOrValue,
};
use primitive_types::U256;
use std::collections::{HashMap, VecDeque};

pub const YOCTO: u128 = 1_000_000_000_000_000_000_000_000;

/// 1E20 yoctoNEAR per byte, or 10kb per NEAR token
pub const STORAGE_AMOUNT_PER_BYTE: u128 = 100_000_000_000_000_000_000;

pub type Epoch = U64;
pub type Block = U64;
pub type StakingPoolAccountId = ValidAccountId;

/// StakingTokenService is used to stake NEAR via delegation to a staking pool and is tracked by
/// STAKE token. STAKE tokens are tracked at the staking pool level, i.e., there is one STAKE token
/// per staking pool. The STAKE token can then be used as a tradeable asset backed by staked NEAR.
/// STAKE tokens can be redeemed for NEAR.
///
/// This contract enables the STAKE owner to earn staking rewards while being able to use it as a tradeable
/// asset without needing to go through the unstaking and withdrawal process first. Thus, the user
/// only needs to retain enough NEAR to buy gas for transactions. Any surplus can be staked and used
/// as a currency backed by staked NEAR. This provides incentive for users to stake their NEAR, which
/// helps to secure the network, and maximizes NEAR value through STAKE tokens.
///
/// contract name: stake.oysterpack.near
///
/// ## Contract Rules
/// - Customers can deposit and stake NEAR in exchange for STAKE tokens
/// - STAKE tokens are issued for NEAR tokens that are delegated to staking pools, i.e., staking NEAR mints STAKE
/// - STAKE tokens implements the allowance-free vault-based token standard
///   - https://github.com/nearprotocol/NEPs/issues/122
///   - https://github.com/near/core-contracts/tree/safe-based-ft/safe-based-fungible-token
/// - STAKE tokens can be redeemed for NEAR which triggers NEAR to be unstaked
/// - STAKE owners can specify the amount of NEAR to unstake for the next scheduled unstaking transaction.
/// - Contract storage fees for the customer account are paid by the customer. The fees are escrowed
///   and refunded as storage goes down.
///
/// ## Customer Account Storage Fees
/// Account storage is dynamically computed at runtime. If account storage increases then escrow
/// deposit is required. If account storage goes down, then escrow funds are refunded.
///
/// The customer will only be charged for storage for storage dedicated to managing the customer account.
/// Global storage fees are paid by OysterPack, e.g., staking pool related storage.
trait StakingService {
    /// Stakes the attached deposit with the specified staking pool.
    /// Returns a Promise with StakeReceipt result
    ///
    /// Any applicable storage account fees will be deducted from the deposit, i.e., the amount stakes
    /// will be less the amount to pay for storage fees.
    ///
    /// ## Workflow
    /// 1. check staking pool account ID - panic if account ID is not valid
    /// 2. check if deposit was attached - panic if deposit == 0
    /// 3. update customer account
    /// 4. check if customer account storage fees need to be applied and deducted from the deposit
    ///    - if there is not enough deposit to cover storage fees, then panic
    /// 3. deposit and stake with the staking pool
    /// 4. refresh OysterPack total staked balance with the staking pool
    /// 5. compute the STAKE token value in NEAR - (total staked token balance) / (total STAKE token supply)
    /// 6. credit the account with STAKE tokens
    /// 7. log the event
    ///
    /// ## Panics
    /// - if [staking_pool_account_id] is not a valid account ID
    /// - if no deposit was attached
    /// - if not enough deposit was made to cover account storage fees
    fn deposit_and_stake(&mut self, staking_pool: StakingPoolAccountId) -> Promise;

    /// Stakes the given NEAR amount with the specified staking pool.
    /// Returns a Promise with StakeReceipt result
    ///
    /// ## Use Case
    /// Can be used to cancel / update queued unstaking receipt. For example:
    /// - customer requests 100 NEAR to be unstaked, which is scheduled to be submitted in 3 epochs
    /// - customer then requests 40 NEAR to be staked, which will update the unstaking receipt debit
    ///   amount to 60 NEAR. Thus, eliminating the need to submit a staking pool request.
    ///
    /// ## Use Case
    /// Customer wants to reposition his stake with different staking pools.
    /// The customer would unstake from one staking pool. When the funds are available, the customer
    /// can stake the funds to another staking pool.
    ///
    /// ## Workflow
    /// 1. check staking pool account ID - panic if account ID is not valid
    /// 2. check customer account unstaked available balance - if balance is too low, then panic
    /// 3. stake specified amount with staking pool
    /// 4. refresh OysterPack total staked balance with the staking pool
    /// 5. compute the STAKE token value in NEAR - (total staked token balance) / (total STAKE token supply)
    /// 6. credit the account with STAKE tokens
    /// 7. log the event
    ///
    /// ## Panics
    /// - If staking pool account ID is not valid
    /// - If the customer available NEAR account balance is too low, i.e., not enough to cover the requested amount to stake.
    fn stake(&mut self, staking_pool: StakingPoolAccountId, amount: YoctoNEAR) -> Promise;

    /// Stakes all available unstaked balance with the specified staking pool.
    /// Returns a Promise with StakeReceipt result
    fn stake_all(&mut self, staking_pool: StakingPoolAccountId) -> Promise;

    /// Unstakes the given yoctoNEAR amount from the customer account.
    /// The customer account should have enough staked balance.
    /// The new total unstaked balance will be available for withdrawal in 4-7 epochs.
    ///
    /// Returns a Promise with UnstakeReceipt result
    ///
    /// ## Panics
    /// - if the staking pool account ID is not valid
    /// - if the current staked balance is not enough to cover the unstake request
    /// - if there are already 3 pending unstake requests
    fn unstake(&mut self, staking_pool: StakingPoolAccountId, amount: YoctoNEAR) -> Promise;

    /// Unstakes all current staked NEAR.
    /// The new total unstaked balance will be available for withdrawal in 4-7 epochs.
    ///
    /// Returns a Promise with UnstakeReceipt result
    ///
    /// ## Panics
    /// - if the staking pool account ID is not valid
    /// - if there are already 3 pending unstake requests
    fn unstake_all(&mut self, staking_pool: StakingPoolAccountId) -> Promise;

    /// Withdraws from the customer's account available NEAR balance.
    ///
    /// ## Notes
    /// - event is logged
    ///
    /// ## Panics
    /// - if account is not registered
    /// - if there are not enough funds in the account to satisfy the withdrawal
    fn withdraw(&mut self, amount: YoctoNEAR);

    /// Withdraws all available NEAR balance.
    ///
    /// Returns Promise with YoctoNEAR result
    ///
    /// ## Notes
    /// - if the account has no stake and has been fully withdrawn, then the account will be auto-deleted
    ///   and the storage fees will be refunded from escrow.
    ///
    /// ## Panics
    /// - if account is not registered
    fn withdraw_all(&mut self) -> Promise;
}

trait StakingPoolRegistry {
    /// Returns information for the specified staking pool.
    /// Returns None if nothing is staked with the specified staking pool.
    ///
    /// NOTE: the info may be out of date - check the epoch heights. To get the latest updated info
    /// use [refresh_stake_info]
    ///
    /// ## Panics
    /// - if account ID is not valid    
    fn staking_pool(&self, staking_pool: StakingPoolAccountId) -> Option<StakingPool>;

    /// Enables a batch of staking pools to be retrieved at once.
    /// If no staking pool was returned for a specified account ID, then it means there is no stake
    /// in it.
    fn staking_pools(&self, staking_pool: Vec<StakingPoolAccountId>) -> Vec<StakingPool>;

    /// Returns staking pool account IDs that are being staked with.
    /// Range can be specified to page through the results.
    fn staking_pool_account_ids(&self, range: Option<Range>) -> Vec<StakingPoolAccountId>;

    /// Returns number of staking pools that are currently staked with across all customer accounts.
    fn staking_pool_count(&self) -> u32;
}

trait StakeToken {
    fn total_supply(&self, staking_pool: StakingPoolAccountId) -> YoctoSTAKE;

    /// Returns how much is 1 STAKE is worth in NEAR
    fn stake_token_value(&self, staking_pool: StakingPoolAccountId) -> Option<StakeTokenValue>;
}

/// Returns the STAKE token value at the specified block height.
/// STAKE token value is computed as [total_staked] / [total_supply]
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeTokenValue {
    pub token_supply: YoctoSTAKE,
    pub staked_balance: YoctoNEAR,
    pub block_height: Block,
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

trait StakeAccountQueryService {
    /// Returns true if the account is registered.
    ///
    /// NOTE: accounts automatically register and unregister, triggered by staking and withdrawal events.
    ///
    /// ## Panics
    /// - if account ID is not valid
    fn account_registered(&self, account_id: ValidAccountId) -> bool;

    /// Returns STAKE balance for the specified account ID within the specified staking pool.
    ///
    /// ## Panics
    /// - if account ID is not valid
    fn account_stake_balance(
        &self,
        account_id: ValidAccountId,
        staking_pool: StakingPoolAccountId,
    ) -> YoctoSTAKE;

    /// Returns STAKE balances for the specified account ID within the specified staking pools.
    /// staking_pools is used to filter staking pool balances. If empty, then all STAKE balances are
    /// returned.
    ///
    /// ## Panics
    /// - if account ID is not valid
    fn account_stake_balances(
        &self,
        account_id: ValidAccountId,
        staking_pools: Vec<StakingPoolAccountId>,
    ) -> HashMap<StakingPoolAccountId, YoctoSTAKE>;

    /// Returns staking pool account IDs that are being staked with.
    /// Range can be specified to page through the results.
    ///
    /// ## Panics
    /// - if account ID is not valid
    fn account_staking_pool_account_ids(
        &self,
        account_id: ValidAccountId,
        range: Option<Range>,
    ) -> Vec<StakingPoolAccountId>;

    /// Returns the number of staking pools for the specified account ID
    ///
    /// ## Panics
    /// - if account ID is not valid
    fn account_staking_pool_count(&self, account_id: ValidAccountId) -> u32;

    /// Returns the amount escrowed to cover account storage fees.
    ///
    /// ## Panics
    /// - if account ID is not valid
    fn account_storage_fee_escrow_near_balance(&self, account_id: ValidAccountId) -> YoctoNEAR;

    /// Returns amount of unstaked NEAR that is available to withdraw.
    ///
    /// ## Panics
    /// - if account ID is not valid
    fn account_available_unstaked_balance(&self, account_id: ValidAccountId) -> YoctoNEAR;

    /// Returns any active unstake receipts. Active receipts are:
    /// 1. funds have been unstaked, and we are waiting for funds to clear to be withdrawn
    /// 2. pending unstaking request that is waiting for unstaked funds to be withdrawn
    ///
    /// ## Panics
    /// - if account ID is not valid
    fn account_active_unstake_receipts(
        &self,
        account_id: ValidAccountId,
        staking_pool: StakingPoolAccountId,
    ) -> Vec<UnstakeReceipt>;
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
#[serde(crate = "near_sdk::serde")]
pub struct Range {
    pub start: Option<u32>,
    pub end: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeReceipt {
    /// amount in yoctoNEAR that was staked
    pub near_tokens_staked: YoctoNEAR,
    /// amount of yoctoSTAKE that was credited to the customer account
    pub stake_credit: YoctoSTAKE,
    /// amount of storage fees that were deducted
    ///
    /// ## Notes
    /// - storage fees will be charged when staking with a new staking pool
    pub storage_fees: Option<YoctoNEAR>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct UnstakeReceipt {
    /// yoctoNEAR
    pub unstaked_near: YoctoNEAR,
    /// yoctoSTAKE that was debited
    pub stake_debit: YoctoSTAKE,
    /// when the unstaked NEAR will be available for withdrawal
    pub epoch_availability: Epoch,
    /// if all funds are unstaked from the staking pool, then storage fees will be refunded from escrow
    /// once the withdrawal is complete
    pub storage_fee_refund: YoctoNEAR,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPool {
    pub account_id: AccountId,
    pub balances: StakingPoolBalances,
    pub token_supply: YoctoSTAKE,
    pub block_height: Block,
}

impl StakingPool {
    pub fn stake_token_value(&self) -> StakeTokenValue {
        StakeTokenValue {
            staked_balance: self.balances.staked,
            token_supply: self.token_supply,
            block_height: self.block_height,
        }
    }
}

/// NEAR transitions: staked -> unstaked -> withdrawn
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolBalances {
    /// staked yoctoNEAR balance
    ///
    /// ## Notes
    /// STAKE tokens are backed by staked NEAR
    pub staked: YoctoNEAR,

    /// unstaked yoctoNEAR balance
    ///
    /// ## Notes
    /// - when NEAR is unstaked, the STAKE token supply is decreased.
    /// - when amount > 0, then it means funds have been unstaked and we are waiting for the funds
    ///   to become available for withdrawal - [unstaked_epoch_height_availability] should specify
    ///   when the funds will become available
    pub unstaked: YoctoNEAR,

    /// when the unstaked NEAR can be withdrawn, i.e., transferred from the staking pool account
    /// to this contract's account
    ///
    /// NOTE: while awaiting for the unstaked balance to clear for withdrawal, unstaking requests
    /// are put on hold and scheduled until the unstaked balance is withdrawn
    pub unstaked_epoch_height_availability: Option<Epoch>,

    /// NEAR that has been unstaked and withdrawn to the contract and now available for withdrawal
    /// by customer accounts
    pub available_unstaked_balance: YoctoNEAR,

    /// How much yoctoNEAR will be unstaked at the next available unstaking epoch.
    /// If balance > 0, then this means we are waiting on currently unstaked NEAR balance withdrawal
    /// to complete, i.e., more NEAR can be unstaked only when the current unstaked balance is zero.
    pub pending_unstaked_balance: YoctoNEAR,
}

#[cfg(test)]
mod test {
    use super::*;
    use near_sdk::borsh::{self, try_from_slice_with_schema, try_to_vec_with_schema};
    use near_sdk::{serde_json, Balance};

    #[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
    #[serde(crate = "near_sdk::serde")]
    struct YoctoNear(U128);

    #[test]
    fn yocto_near_json() {
        let near = YoctoNear(100.into());
        let json = serde_json::to_string_pretty(&near).unwrap();
        println!("[{}]", json);

        let near2: YoctoNear = serde_json::from_str(&json).unwrap();
        assert_eq!(near, near2);
    }

    #[test]
    fn hashbrown_map_json() {
        let mut map = HashMap::<AccountId, Balance>::new();
        map.insert("staking-pool.near".to_string(), 100);
        let json = serde_json::to_string_pretty(&map).unwrap();
        println!("map: {}", json);
    }

    use borsh::{BorshDeserialize, BorshSerialize};

    #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
    struct A {
        x: u64,
        y: String,
        z: u128,
    }

    #[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
    struct A2 {
        x: u64,
        y: String,
        z: u128,
    }

    #[test]
    fn test_simple_struct() {
        let a = A2 {
            x: 3301,
            y: "liber primus".to_string(),
            z: 10,
        };
        let encoded_a = a.try_to_vec().unwrap();
        let decoded_a2 = A::try_from_slice(&encoded_a).unwrap();
        println!("{:?}", decoded_a2);
    }
}
