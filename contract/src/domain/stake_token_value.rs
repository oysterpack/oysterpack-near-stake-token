use crate::domain::{
    BlockTimeHeight, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear, YoctoStake,
};
use crate::near::YOCTO;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use primitive_types::U256;

/// STAKE token value at a point in time, i.e., at a block height.
///
/// STAKE token value = [total_staked_near_balance] / [total_stake_supply]
///
/// NOTE: The STAKE token value is gathered while the contract is locked.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StakeTokenValue {
    block_time_height: BlockTimeHeight,
    total_staked_near_balance: YoctoNear,
    total_stake_supply: YoctoStake,
}

impl StakeTokenValue {
    /// [clock_time_height] is retrieved from the NEAR runtime env
    ///
    /// ## Panics
    /// - if NEAR runtime env is not available
    /// - if only 1 of the balances is zero
    pub fn new(total_staked_near_balance: YoctoNear, total_stake_supply: YoctoStake) -> Self {
        if total_staked_near_balance.value() == 0 {
            assert_eq!(
                total_stake_supply.value(),
                0,
                "if NEAR balance is zero, then STAKE supply must be zero"
            )
        }
        if total_stake_supply.value() == 0 {
            assert_eq!(
                total_staked_near_balance.value(),
                0,
                "if STAKE supply is zero, then NEAR balance  must be zero"
            )
        }
        Self {
            block_time_height: BlockTimeHeight::from_env(),
            total_stake_supply,
            total_staked_near_balance,
        }
    }

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
            return YOCTO.into();
        }
        let value = U256::from(near) * U256::from(self.total_stake_supply)
            / U256::from(self.total_staked_near_balance);
        value.as_u128().into()
    }

    pub fn stake_to_near(&self, stake: YoctoStake) -> YoctoNear {
        if self.total_staked_near_balance.value() == 0 || self.total_stake_supply.value() == 0 {
            return YOCTO.into();
        }
        let value = U256::from(stake) * U256::from(self.total_staked_near_balance)
            / U256::from(self.total_stake_supply);
        value.as_u128().into()
    }

    /// returns the value of 1 STAKE token
    pub fn value(&self) -> YoctoNear {
        self.stake_to_near(YoctoStake(YOCTO))
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::test_utils::near::*;
    use near_sdk::{serde_json, testing_env, AccountId, MockedBlockchain, VMContext};

    #[test]
    fn when_total_stake_supply_is_zero() {
        let account_id = "bob.near";
        let context = new_context(account_id);
        testing_env!(context);

        let stake_token_value = StakeTokenValue::new(YoctoNear(0), YoctoStake(0));
        assert_eq!(stake_token_value.value(), YoctoNear(YOCTO))
    }
}
