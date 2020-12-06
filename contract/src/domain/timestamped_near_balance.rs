use crate::domain::{BlockHeight, BlockTimestamp, EpochHeight, YoctoNear};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
};
use std::cmp::Ordering;

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, Default)]
pub struct TimestampedNearBalance {
    balance: YoctoNear,
    block_height: BlockHeight,
    block_timestamp: BlockTimestamp,
    epoch_height: EpochHeight,
}

impl PartialEq for TimestampedNearBalance {
    fn eq(&self, other: &Self) -> bool {
        self.balance == other.balance
    }
}

impl PartialEq<u128> for TimestampedNearBalance {
    fn eq(&self, other: &u128) -> bool {
        self.balance.0 == *other
    }
}

impl PartialOrd for TimestampedNearBalance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.balance.cmp(&other.balance))
    }
}

impl PartialOrd<u128> for TimestampedNearBalance {
    fn partial_cmp(&self, other: &u128) -> Option<Ordering> {
        Some(self.balance.0.cmp(other))
    }
}

impl TimestampedNearBalance {
    /// [block_height], [block_timestamp], and [epoch_height] are initialized from the NEAR runtime
    /// environment
    ///
    /// ## Panics
    /// if NEAR runtime context is not available
    pub fn new(balance: YoctoNear) -> Self {
        Self {
            balance,
            block_height: env::block_index().into(),
            block_timestamp: env::block_timestamp().into(),
            epoch_height: env::epoch_height().into(),
        }
    }

    pub fn balance(&self) -> YoctoNear {
        self.balance
    }

    pub fn block_height(&self) -> BlockHeight {
        self.block_height
    }

    pub fn block_timestamp(&self) -> BlockTimestamp {
        self.block_timestamp
    }

    pub fn epoch_height(&self) -> EpochHeight {
        self.epoch_height
    }

    /// ## Panics
    /// if overflow occurs
    pub fn credit(&mut self, amount: YoctoNear) {
        if amount.0 == 0 {
            return;
        }
        self.balance = self
            .balance
            .0
            .checked_add(amount.0)
            .expect(
                format!(
                    "credit caused balance to overflow: {balance} + {amount}",
                    amount = amount,
                    balance = self.balance
                )
                .as_str(),
            )
            .into();
        self.update_timestamp();
    }

    /// ## Panics
    /// if debit amount > balance
    pub fn debit(&mut self, amount: YoctoNear) {
        if amount.0 == 0 {
            return;
        }
        assert!(
            self.balance >= amount,
            "debit amount cannot be greater than the current balance: {} - {}",
            self.balance,
            amount,
        );
        self.balance.0 -= amount.0;
        self.update_timestamp();
    }

    fn update_timestamp(&mut self) {
        self.epoch_height = env::epoch_height().into();
        self.block_timestamp = env::block_timestamp().into();
        self.block_height = env::block_index().into();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::near::new_context;
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    #[should_panic]
    fn timestamped_balance_new_outside_near_runtime() {
        let _balance = TimestampedNearBalance::new(10.into());
    }

    #[test]
    fn timestamped_balance_new() {
        let mut context = new_context("bob.near");
        context.block_index = 1;
        context.block_timestamp = 2;
        context.epoch_height = 3;

        testing_env!(context);
        let balance = TimestampedNearBalance::new(10.into());
        assert_eq!(balance.balance(), 10.into());
        assert_eq!(balance.block_height(), 1.into());
        assert_eq!(balance.block_timestamp(), 2.into());
        assert_eq!(balance.epoch_height(), 3.into());
    }

    #[test]
    pub fn timestamped_balance_partial_eq() {
        let mut context = new_context("bob.near");
        testing_env!(context.clone());

        let balance_1 = TimestampedNearBalance::new(10.into());

        context.block_index = 10;
        context.block_timestamp = 20;
        context.epoch_height = 30;
        testing_env!(context.clone());
        let balance_2 = TimestampedNearBalance::new(10.into());

        assert!(balance_1 == balance_2);
        assert!(balance_1 == 10u128);
    }

    #[test]
    pub fn timestamped_balance_debug() {
        let mut context = new_context("bob.near");
        context.block_index = 1;
        context.block_timestamp = 2;
        context.epoch_height = 3;
        testing_env!(context.clone());

        let balance = TimestampedNearBalance::new(10.into());
        println!("{:?}", balance);
    }

    #[test]
    pub fn timestamped_balance_borsh() {
        let mut context = new_context("bob.near");
        context.block_index = 1;
        context.block_timestamp = 2;
        context.epoch_height = 3;
        testing_env!(context.clone());

        let balance = TimestampedNearBalance::new(10.into());
        let bytes: Vec<u8> = balance.try_to_vec().unwrap();
        let balance2: TimestampedNearBalance =
            TimestampedNearBalance::try_from_slice(&bytes).unwrap();
        assert_eq!(balance, balance2);
    }
}
