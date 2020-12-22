use crate::domain::{BlockTimeHeight, YoctoNear, YoctoStake};
use crate::near::YOCTO;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use primitive_types::U256;

/// STAKE token value at a point in time, i.e., at a block height.
///
/// STAKE token value = [total_staked_near_balance] / [total_stake_supply]
///
/// NOTE: The STAKE token value is gathered while the contract is locked.
#[derive(BorshSerialize, BorshDeserialize, Copy, Clone, Default)]
pub struct StakeTokenValue {
    pub block_time_height: BlockTimeHeight,
    pub total_staked_near_balance: YoctoNear,
    pub total_stake_supply: YoctoStake,
}

impl StakeTokenValue {
    pub fn block_time_height(&self) -> BlockTimeHeight {
        self.block_time_height
    }

    pub fn total_staked_near_balance(&self) -> YoctoNear {
        self.total_staked_near_balance
    }

    pub fn total_stake_supply(&self) -> YoctoStake {
        self.total_stake_supply
    }

    /// converts NEAR to STAKE rounded down
    pub fn near_to_stake(&self, near: YoctoNear) -> YoctoStake {
        if self.total_staked_near_balance.value() == 0 || self.total_stake_supply.value() == 0 {
            return near.value().into();
        }
        let value = U256::from(near) * U256::from(self.total_stake_supply)
            / U256::from(self.total_staked_near_balance);
        value.as_u128().into()
    }

    pub fn stake_to_near(&self, stake: YoctoStake) -> YoctoNear {
        if self.total_staked_near_balance.value() == 0
            || self.total_stake_supply.value() == 0
            // TODO: when deposit and staked with staking pool, there is a small amount remaining as unstaked
            //       however, STAKE token value can never be less than 1:1 in terms of NEAR
            || self.total_staked_near_balance.value() < self.total_stake_supply.value()
        {
            return stake.value().into();
        }
        let value = U256::from(stake) * U256::from(self.total_staked_near_balance)
            / U256::from(self.total_stake_supply);
        value.as_u128().into()
    }

    /// returns the value of 1 STAKE token
    pub fn value(&self) -> YoctoNear {
        self.stake_to_near(YoctoStake(YOCTO))
    }

    /// returns true if the current epoch height is greater than the epoch height when the stake
    /// token value was last computed
    pub fn is_stale(&self) -> bool {
        near_sdk::env::epoch_height() > self.block_time_height.epoch_height().value()
    }

    /// returns true if the stake token value was computed within the same epoch period
    pub fn is_current(&self) -> bool {
        near_sdk::env::epoch_height() == self.block_time_height.epoch_height().value()
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    fn when_total_stake_supply_is_zero() {
        let account_id = "bob.near";
        let context = new_context(account_id);
        testing_env!(context);

        let stake_token_value = StakeTokenValue::default();
        assert_eq!(stake_token_value.value(), YoctoNear(YOCTO));

        assert_eq!(
            stake_token_value.stake_to_near((10 * YOCTO).into()),
            YoctoNear(10 * YOCTO)
        );
        assert_eq!(
            stake_token_value.near_to_stake((10 * YOCTO).into()),
            YoctoStake(10 * YOCTO)
        );
    }
}
