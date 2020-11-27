use crate::state;
use hashbrown::HashMap;
use near_sdk::json_types::U64;
use near_sdk::{
    json_types::U128,
    serde::{Deserialize, Serialize},
    AccountId, Balance, EpochHeight, PromiseOrValue,
};

/// Units
/// - yoctoSTAKE - smallest non-divisible amount of STAKE token.
/// - STAKE - 10^24 yoctoSTAKE
pub const STAKE_YOCTO_UNITS: u128 = 1_000_000_000_000_000_000_000_000;

/// OysterPack STAKE token is money backed by staked NEAR tokens.
///
/// contract name: stake.oysterpack.near
///
/// # Contract Rules
/// - Customers can deposit and stake NEAR in exchange for STAKE tokens
/// - STAKE tokens are issued for NEAR tokens that are delegated to staking pools
/// - STAKE tokens can only be redeemed, i.e., unstaked, for NEAR tokens at predefined intervals defined
///   at the global contract level. The interval will every 180 epochs starting from the contract
///   deployment epoch height. This corresponds to every ~90 days, i.e., quarterly.
/// - Customers can specify the amount of NEAR to unstake for the next scheduled unstaking transaction.
/// - Contract storage fees are paid from stake rewards.
trait OysterPackStakeToken {
    /// Stakes the attached deposit with the specified staking pool.
    /// Returns the amount of yoctoSTAKE that is credited to the account.
    ///
    /// Any applicable storage account fees will be deducted from the deposit.
    ///
    /// ## Workflow
    /// 1. check staking pool account ID - panic if account ID is not valid
    /// 2. check if deposit was attached - panic if deposit == 0
    /// 2. deposit and stake the deposited amount with the staking pool
    /// 3. refresh OysterPack total staked balance with the staking pool
    /// 4. compute the STAKE token value in NEAR - (total staked token balance) / (total STAKE token supply)
    /// 5. credit the account with STAKE tokens
    /// 6. log the event
    ///
    /// ## Panics
    /// - if [staking_pool_account_id] is not a valid account ID
    /// - if no deposit was attached
    fn deposit_and_stake(&mut self, staking_pool_account_id: AccountId) -> U128;

    /// Stakes the given amount with the specified staking pool.
    /// Returns the number of STAKE tokens credited to the customer account.
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
    fn stake(&mut self, staking_pool_account_id: AccountId, amount: U128) -> U128;

    /// Stakes all available unstaked balance with the specified staking pool.
    fn stake_all(&mut self, staking_pool_account_id: AccountId) -> U128;

    /// Unstakes the given yoctoNEAR amount from the customer account.
    /// The customer account should have enough staked balance.
    /// The new total unstaked balance will be available for withdrawal in 4-7 epochs.
    ///
    /// ## Panics
    /// - if the staking pool account ID is not valid
    /// - if the current staked balance is not enough to cover the unstake request
    /// - if there are already 3 pending unstake requests
    fn unstake(&mut self, staking_pool_account_id: AccountId, amount: U128) -> UnstakeRequest;

    /// Unstakes all current staked NEAR.
    /// The new total unstaked balance will be available for withdrawal in 4-7 epochs.
    ///
    /// There is a limit of 3 pending unstake requests.
    ///
    /// ## Panics
    /// - if the staking pool account ID is not valid
    /// - if the current staked balance is not enough to cover the unstake request
    /// - if there are already 3 pending unstake requests
    fn unstake_all(&mut self, staking_pool_account_id: AccountId) -> UnstakeRequest;

    /// Returns information for the specified staking pool.
    /// Returns None if nothing is staked with the specified staking pool.
    ///
    /// NOTE: the info may be out of date - check the epoch heights. To get the latest updated info
    /// use [refresh_stake_info]
    ///
    /// ## Panics
    /// - if account ID is not valid    
    fn staking_pool(&self, account_id: AccountId) -> Option<StakingPool>;

    /// Returns the yoctoSTAKE token total supply for each staking pool account
    fn stake_token_total_supply(&self) -> HashMap<AccountId, U128>;
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPool {
    pub account_id: AccountId,
    pub balances: StakingPoolBalances,
    pub token_supply: U128,
    pub block_height: U64,
}

/// NEAR transitions: staked -> unstaked -> withdrawn
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolBalances {
    /// staked yoctoNEAR balance
    ///
    /// ## Notes
    /// STAKE tokens are backed by staked NEAR
    pub staked: U128,
    /// unstaked yoctoNEAR balance
    ///
    /// ## Notes
    /// When NEAR is unstaked, the STAKE token supply is decreased.
    pub unstaked: U128,
    /// when the unstaked NEAR can be withdrawn, i.e., transferred from the staking pool account
    /// to this contract's account
    ///
    /// NOTE: while awaiting for the unstaked balance to clear for withdrawal, unstaking requests
    /// are put on hold and scheduled until the unstaked balance is withdrawn
    pub unstaked_epoch_height_availability: Option<U64>,
    /// NEAR that has been unstaked and withdrawn to the contract and now available for withdrawal
    /// by customer accounts
    pub withdrawn: U128,

    /// How much yoctoNEAR will be unstaked at the next available unstaking epoch.
    /// If balance > 0, then this means we are waiting on currently unstaked NEAR balance withdrawal
    /// to complete, i.e., more NEAR can be unstaked only when the current unstaked balance is zero.
    pub pending_unstaked_balance: Balance,
}

// impl From<state::Stake> for StakingPool {
//     fn from(stake: state::Stake) -> Self {
//         Self {
//             account_id: stake.staking_pool().to_string(),
//             balances: StakingPoolBalances {
//                 staked: stake.balances().staked().into(),
//                 unstaked: stake.balances().unstaked().into(),
//                 unstaked_epoch_height_availability: stake
//                     .balances()
//                     .unstaked_epoch_height_availability()
//                     .map(|epoch_height| epoch_height.into()),
//             },
//             token_supply: stake.token_supply().into(),
//             block_height: stake.block_height().into(),
//         }
//     }
// }

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeOwnerAccount {
    pub account_id: AccountId,
    pub staking_pool_accounts: HashMap<AccountId, StakingPoolAccount>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakingPoolAccount {
    /// yoctoSTAKE
    pub stake_token_balance: Balance,
    /// unstake request that has been scheduled because it is waiting for a previous unstake request
    /// to complete at the staking pool level
    pub unstake_request_scheduled: Option<UnstakeRequest>,
    /// unstake requests that have been submitted to the staking pool and waiting for the NEAR funcs
    /// to become available for withdrawal
    pub unstake_request_in_progress: Option<UnstakeRequest>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct UnstakeRequest {
    /// yoctoNEAR
    pub balance: Balance,
    /// when the unstaked NEAR will be available for withdrawal
    pub epoch_availability: U64,
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
