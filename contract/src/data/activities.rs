use crate::data::TimestampedBalance;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::EpochHeight;
use std::ops::{Deref, DerefMut};

/// Tracks `deposit_and_stake` staking pool function calls.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct DepositAndStake(TimestampedBalance);

impl Deref for DepositAndStake {
    type Target = TimestampedBalance;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DepositAndStake {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct Unstake(TimestampedBalance);

impl Deref for Unstake {
    type Target = TimestampedBalance;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Unstake {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct WithdrawAll {
    /// funds will be available for withdrawal at the specified epoch height
    epoch_height_availability: EpochHeight,
}

impl WithdrawAll {
    pub fn epoch_height_availability(&self) -> EpochHeight {
        self.epoch_height_availability
    }

    pub fn set_epoch_height_availability(&mut self, epoch_height: EpochHeight) -> &mut Self {
        self.epoch_height_availability = epoch_height;
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::near::new_context;
    use near_sdk::{testing_env, MockedBlockchain, VMContext};

    #[test]
    fn deposit_and_stake_deref() {
        let context = new_context("bob.near".to_string());
        testing_env!(context);
        let mut activity = DepositAndStake(TimestampedBalance::new(1000));

        fn foo(balance: &TimestampedBalance) {
            println!("{:?}", balance);
        }

        fn bar(balance: &mut TimestampedBalance) {
            balance.credit(500);
            println!("{:?}", balance);
        }

        foo(&activity);
        bar(&mut activity);
        assert_eq!(activity.balance, 1500);
    }
}
