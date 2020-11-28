use crate::state;
use near_sdk::{
    json_types::{U128, U64},
    serde::{Deserialize, Serialize},
    AccountId, Balance, EpochHeight, PromiseOrValue,
};
use std::collections::HashMap;

/// Units
/// - yoctoSTAKE - smallest non-divisible amount of STAKE token.
/// - STAKE - 10^24 yoctoSTAKE
pub const STAKE_YOCTO_UNITS: u128 = 1_000_000_000_000_000_000_000_000;

/// 1E20 yoctoNEAR per byte, or 10kb per NEAR token
pub const STORAGE_AMOUNT_PER_BYTE: u128 = 100_000_000_000_000_000_000;

pub type YoctoNEAR = U128;
pub type YoctoSTAKE = U128;
pub type Epoch = U64;
pub type Block = U64;

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
trait StakingTokenService {
    /// Stakes the attached deposit with the specified staking pool.
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
    fn deposit_and_stake(&mut self, staking_pool_account_id: AccountId) -> StakeReceipt;

    /// Stakes the given NEAR amount with the specified staking pool.
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
    fn stake(&mut self, staking_pool_account_id: AccountId, amount: YoctoNEAR) -> StakeReceipt;

    /// Stakes all available unstaked balance with the specified staking pool.
    fn stake_all(&mut self, staking_pool_account_id: AccountId) -> StakeReceipt;

    /// Unstakes the given yoctoNEAR amount from the customer account.
    /// The customer account should have enough staked balance.
    /// The new total unstaked balance will be available for withdrawal in 4-7 epochs.
    ///
    /// ## Panics
    /// - if the staking pool account ID is not valid
    /// - if the current staked balance is not enough to cover the unstake request
    /// - if there are already 3 pending unstake requests
    fn unstake(&mut self, staking_pool_account_id: AccountId, amount: YoctoNEAR) -> UnstakeReceipt;

    /// Unstakes all current staked NEAR.
    /// The new total unstaked balance will be available for withdrawal in 4-7 epochs.
    ///
    /// There is a limit of 3 pending unstake requests.
    ///
    /// ## Panics
    /// - if the staking pool account ID is not valid
    /// - if the current staked balance is not enough to cover the unstake request
    /// - if there are already 3 pending unstake requests
    fn unstake_all(&mut self, staking_pool_account_id: AccountId) -> UnstakeReceipt;

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
    /// ## Notes
    /// - if the account has no stake and has been fully withdrawn, then the account will be auto-deleted
    ///   and the storage fees will be refunded from escrow.
    ///
    /// ## Panics
    /// - if account is not registered
    fn withdraw_all(&mut self) -> YoctoNEAR;

    /// Returns information for the specified staking pool.
    /// Returns None if nothing is staked with the specified staking pool.
    ///
    /// NOTE: the info may be out of date - check the epoch heights. To get the latest updated info
    /// use [refresh_stake_info]
    ///
    /// ## Panics
    /// - if account ID is not valid    
    fn staking_pool(&self, staking_pool_account_id: AccountId) -> Option<StakingPool>;

    /// Returns staking pool account IDs that are being staked with
    fn staking_pool_account_ids(&self) -> Vec<AccountId>;

    fn staking_pool_count(&self) -> u32;

    /// Looks up and returns account info using predecessor account ID
    fn stake_account(&self) -> Option<StakeOwnerAccount>;
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
    /// all funds are unstaked from the staking pool, then storage fees will be refunded from escrow
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
    /// When NEAR is unstaked, the STAKE token supply is decreased.
    pub unstaked: YoctoNEAR,
    /// when the unstaked NEAR can be withdrawn, i.e., transferred from the staking pool account
    /// to this contract's account
    ///
    /// NOTE: while awaiting for the unstaked balance to clear for withdrawal, unstaking requests
    /// are put on hold and scheduled until the unstaked balance is withdrawn
    pub unstaked_epoch_height_availability: Option<Epoch>,
    /// NEAR that has been unstaked and withdrawn to the contract and now available for withdrawal
    /// by customer accounts
    pub withdrawn: YoctoNEAR,

    /// How much yoctoNEAR will be unstaked at the next available unstaking epoch.
    /// If balance > 0, then this means we are waiting on currently unstaked NEAR balance withdrawal
    /// to complete, i.e., more NEAR can be unstaked only when the current unstaked balance is zero.
    pub pending_unstaked_balance: YoctoNEAR,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeOwnerAccount {
    pub account_id: AccountId,
    pub staking_pool_accounts: HashMap<AccountId, StakingPoolAccount>,
    /// NEAR that has been escrowed for account storage fees
    pub storage_fee_escrow_balance: YoctoNEAR,
    /// NEAR that is available for withdrawal
    ///
    /// ## Notes
    /// - credits from funds that are unstaked and withdrawn from staling pool - [UnstakeReceipt]
    /// - debits from customer withdrawals, i.e., NEAR is transferred out to customer account
    pub near_available_balance: YoctoNEAR,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolAccount {
    /// yoctoSTAKE
    pub stake_token_balance: YoctoSTAKE,
    /// unstake request that has been scheduled because it is waiting for a previous unstake request
    /// to complete at the staking pool level
    pub unstake_request_scheduled: Option<UnstakeReceipt>,
    /// unstake requests that have been submitted to the staking pool and waiting for the NEAR funcs
    /// to become available for withdrawal
    pub unstake_request_in_progress: Option<UnstakeReceipt>,
}

#[cfg(test)]
mod test {
    use super::*;
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
}
