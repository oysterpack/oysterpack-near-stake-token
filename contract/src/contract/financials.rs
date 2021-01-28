use crate::interface::{
    BlockHeight, BlockTimestamp, ContractBalances, ContractFinancials, EarningsDistribution,
};

//required in order for near_bindgen macro to work outside of lib.rs
use crate::config::CONTRACT_MIN_OPERATIONAL_BALANCE;
use crate::near::log;
use crate::*;
use near_sdk::near_bindgen;

// YBHdPpe867!KMFTvFSTWbnxy
#[near_bindgen]
impl ContractFinancials for StakeTokenContract {
    fn balances(&self) -> ContractBalances {
        ContractBalances {
            total_contract_balance: env::account_balance().into(),
            total_contract_storage_usage_cost: self.total_contract_storage_usage_cost().into(),
            total_available_balance: self.total_available_balance().into(),

            total_user_accounts_balance: self.total_user_accounts_balance().into(),
            customer_batched_stake_deposits: self.customer_batched_stake_deposits().into(),
            total_available_unstaked_near: self.total_near.amount().into(),
            near_liquidity_pool: self.near_liquidity_pool.into(),
            total_account_storage_escrow: self.total_account_storage_escrow.into(),

            contract_owner_storage_usage_cost: self.contract_owner_storage_usage_cost().into(),
            contract_owner_available_balance: self.owner_available_balance().into(),

            contract_owner_balance: self.contract_owner_balance.into(),
            contract_earnings: self.contract_earnings().into(),
            contract_owner_earnings: self.contract_owner_earnings().into(),
            user_accounts_earnings: self.user_accounts_earnings().into(),
            collected_earnings: self.collected_earnings.into(),

            contract_required_operational_balance: CONTRACT_MIN_OPERATIONAL_BALANCE.into(),

            block_height: BlockHeight(env::block_index().into()),
            block_timestamp: BlockTimestamp(env::block_timestamp().into()),
        }
    }

    #[payable]
    fn deposit_earnings(&mut self) -> interface::YoctoNear {
        *self.collected_earnings += env::account_balance();
        self.collected_earnings.into()
    }
}

impl StakeTokenContract {
    pub fn total_contract_storage_usage_cost(&self) -> YoctoNear {
        (env::storage_usage() as u128 * self.config.storage_cost_per_byte().value()).into()
    }

    pub fn total_available_balance(&self) -> YoctoNear {
        (env::account_balance() - self.total_contract_storage_usage_cost().value()).into()
    }

    pub fn customer_batched_stake_deposits(&self) -> YoctoNear {
        (self
            .stake_batch
            .map_or(0, |batch| batch.balance().amount().value())
            + self
                .next_stake_batch
                .map_or(0, |batch| batch.balance().amount().value()))
        .into()
    }

    pub fn total_user_accounts_balance(&self) -> YoctoNear {
        (self.customer_batched_stake_deposits().value()
            + self.total_near.amount().value()
            + self.near_liquidity_pool.value()
            + self.total_account_storage_escrow.value())
        .into()
    }

    /// returns how much gas rewards the contract has accumulated
    pub fn contract_earnings(&self) -> YoctoNear {
        env::account_balance()
            .saturating_sub(self.contract_owner_balance.value())
            .saturating_sub(self.total_user_accounts_balance().value())
            .saturating_sub(self.collected_earnings.value())
            .into()
    }

    pub fn total_earnings(&self) -> YoctoNear {
        self.contract_earnings() + self.collected_earnings
    }

    /// percentage of earnings from contract gas rewards and collected earnings that are allotted to
    /// the contract owner
    pub fn contract_owner_earnings(&self) -> YoctoNear {
        self.contract_owner_share(self.total_earnings())
    }

    fn contract_owner_share(&self, amount: YoctoNear) -> YoctoNear {
        let contract_owner_earnings_percentage =
            self.config.contract_owner_earnings_percentage() as u128;
        (amount.value() / 100 * contract_owner_earnings_percentage).into()
    }

    pub fn user_accounts_earnings(&self) -> YoctoNear {
        self.total_earnings() - self.contract_owner_earnings()
    }

    pub fn contract_owner_storage_usage_cost(&self) -> YoctoNear {
        (self.contract_initial_storage_usage.value() as u128
            * self.config.storage_cost_per_byte().value())
        .into()
    }

    pub fn owner_available_balance(&self) -> YoctoNear {
        let balance = self.contract_owner_balance - self.contract_owner_storage_usage_cost();
        if balance > CONTRACT_MIN_OPERATIONAL_BALANCE {
            balance - CONTRACT_MIN_OPERATIONAL_BALANCE
        } else {
            0.into()
        }
    }

    pub fn distribute_earnings(&mut self) {
        let contract_owner_earnings = self.contract_owner_earnings();
        let user_accounts_earnings = self.user_accounts_earnings();

        self.contract_owner_balance = self
            .contract_owner_balance
            .saturating_add(contract_owner_earnings.value())
            .into();

        // funds added to liquidity pool distributes earnings to the user
        self.near_liquidity_pool = self
            .near_liquidity_pool
            .saturating_add(user_accounts_earnings.value())
            .into();

        // collected earnings have been distributed
        self.collected_earnings = 0.into();

        log(EarningsDistribution {
            contract_owner_earnings: contract_owner_earnings.into(),
            user_accounts_earnings: user_accounts_earnings.into(),
        })
    }
}
