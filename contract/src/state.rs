use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    AccountId, Balance, BlockHeight, EpochHeight,
};
use primitive_types::U256;
use std::collections::HashMap;

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone)]
pub struct Stake {
    staking_pool: AccountId,
    balances: StakeBalances,
    token_supply: Balance,
    block_height: BlockHeight,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone, Copy, Default)]
pub struct StakeBalances {
    staked: Balance,
    unstaked: Balance,
    unstaked_epoch_height_availability: Option<EpochHeight>,
}

impl StakeBalances {
    pub fn new(
        staked: Balance,
        unstaked: Balance,
        unstaked_epoch_height_availability: Option<EpochHeight>,
    ) -> Self {
        Self {
            staked,
            unstaked,
            unstaked_epoch_height_availability,
        }
    }

    /// Returns yoctoNEAR that is staked
    pub fn staked(&self) -> Balance {
        self.staked
    }

    /// Returns yoctoNEAR that is unstaked
    pub fn unstaked(&self) -> Balance {
        self.unstaked
    }

    /// If unstaked balance > 0, then this is when the available balance will be available to withdraw
    /// from the staking pool.
    ///
    /// Returns None if the unstaked balance is zero.
    pub fn unstaked_epoch_height_availability(&self) -> Option<EpochHeight> {
        self.unstaked_epoch_height_availability
    }
}

impl Stake {
    pub fn new(staking_pool: AccountId) -> Stake {
        Stake {
            staking_pool,
            balances: Default::default(),
            token_supply: 0,
            block_height: 0,
        }
    }

    pub fn staking_pool(&self) -> &AccountId {
        &self.staking_pool
    }

    pub fn balances(&self) -> StakeBalances {
        self.balances
    }

    pub fn token_supply(&self) -> Balance {
        self.token_supply
    }

    pub fn block_height(&self) -> BlockHeight {
        self.block_height
    }

    pub fn update_staking_pool_account_balances(
        self,
        balances: StakeBalances,
        block_height: BlockHeight,
    ) -> Stake {
        self.assert_block_height(block_height);
        Stake {
            balances,
            block_height,
            ..self
        }
    }

    fn assert_block_height(&self, block_height: BlockHeight) {
        assert!(
            block_height >= self.block_height,
            "block height is outdated: {} < {}",
            block_height,
            self.block_height
        );
    }

    /// ## Panics
    /// - if block height is outdated, i.e., [block_height] < [Stake::stake_token_supply_epoch]
    /// - if adding the stake amount results in an overflow
    pub fn inc_stake(self, stake_amount: Balance, block_height: BlockHeight) -> Stake {
        self.assert_block_height(block_height);
        let token_supply = self.token_supply + stake_amount;
        assert!(
            token_supply >= self.token_supply,
            "stake amount overflowed: {} + {}",
            self.token_supply,
            stake_amount
        );
        Stake {
            token_supply,
            block_height,
            ..self
        }
    }

    pub fn dec_stake(self, stake_amount: Balance, block_height: BlockHeight) -> Stake {
        self.assert_block_height(block_height);
        assert!(
            stake_amount <= self.token_supply,
            "stake token supply cannot go negative"
        );
        Stake {
            token_supply: self.token_supply - stake_amount,
            block_height,
            ..self
        }
    }

    /// Computes the STAKE token value = [staked_balance] / [stake_token_supply]
    pub fn stake_token_value(&self) -> u128 {
        if self.token_supply == 0 {
            return 1;
        }
        self.balances.staked / self.token_supply
    }

    /// Returns the number of yoctoSTAKE tokens rounded down corresponding to the given yoctoNEAR
    /// balance amount.
    pub fn near_to_stake(&self, near_amount: Balance) -> u128 {
        if self.token_supply == 0 {
            return near_amount;
        }
        let value = U256::from(self.balances.staked) * U256::from(near_amount)
            / U256::from(self.token_supply);
        value.as_u128()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::stake::STAKE_YOCTO_UNITS;
    use std::{
        convert::TryFrom,
        io::{self, Write},
    };

    #[test]
    fn stake_new() {
        let stake = Stake::new("account_id".to_string());
        assert_eq!(stake.staking_pool, "account_id");

        assert_eq!(stake.balances.staked, 0);
        assert_eq!(stake.balances.unstaked, 0);
        assert!(stake.balances.unstaked_epoch_height_availability.is_none());

        assert_eq!(stake.token_supply, 0);
        assert_eq!(stake.block_height, 0);

        assert_eq!(stake.stake_token_value(), 1);
        assert_eq!(
            stake.near_to_stake(10 * STAKE_YOCTO_UNITS),
            10 * STAKE_YOCTO_UNITS
        );
    }

    #[test]
    fn stake_token_value() {
        let stake = Stake::new("account_id".to_string());
        let stake = stake.update_staking_pool_account_balances(
            StakeBalances {
                staked: 100,
                unstaked: 10,
                unstaked_epoch_height_availability: Some(200),
            },
            50,
        );
    }
}
