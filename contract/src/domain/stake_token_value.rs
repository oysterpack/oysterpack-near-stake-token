use crate::{
    domain::{BlockTimeHeight, YoctoNear, YoctoStake},
    interface::{self, staking_service::events},
    near::log,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use primitive_types::U256;

/// STAKE token value at a point in time, i.e., at a block height.
///
/// STAKE token value = [total_staked_near_balance] / [total_stake_supply]
///
/// NOTE: The STAKE token value is gathered while the contract is locked.
#[derive(BorshSerialize, BorshDeserialize, Copy, Clone, Default, Debug)]
pub struct StakeTokenValue {
    block_time_height: BlockTimeHeight,
    total_staked_near_balance: YoctoNear,
    total_stake_supply: YoctoStake,
}

impl StakeTokenValue {
    pub fn new(
        block_time_height: BlockTimeHeight,
        total_staked_near_balance: YoctoNear,
        total_stake_supply: YoctoStake,
    ) -> Self {
        // When staked with staking pool, the staking pool converts the NEAR into shares. However,
        // any fractional amount cannot be staked, and remains in the unstaked balance. For example:
        //
        // total_staked_near_balance:  '999999999999999999999994',
        // total_stake_supply:        '1000000000000000000000000'
        //
        // However, the STAKE token value can never be < 1 NEAR token. Thus, this will compensate
        // when the deposits are first staked. Overtime, as staking rewards accumulate, they will
        // mask the fractional share accounting issue.
        //
        // NOTE: we are talking practically 0 value on the yocto scale.
        if total_staked_near_balance.value() < total_stake_supply.value() {
            Self {
                block_time_height,
                total_staked_near_balance: total_stake_supply.value().into(),
                total_stake_supply,
            }
        } else {
            Self {
                block_time_height,
                total_staked_near_balance,
                total_stake_supply,
            }
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

    /// converts NEAR to STAKE rounded down and then adds back the remainder
    /// - the remainder is added back at a 1:1 ratio because i yoctoSTAKE is the smallest unit and is
    ///   not further divisible
    pub fn near_to_stake(&self, near: YoctoNear) -> YoctoStake {
        if self.total_staked_near_balance.value() == 0 || self.total_stake_supply.value() == 0 {
            return near.value().into();
        }

        let near = U256::from(near);
        let total_stake_supply = U256::from(self.total_stake_supply);
        let total_staked_near_balance = U256::from(self.total_staked_near_balance);

        let stake_value = near * total_stake_supply / total_staked_near_balance;

        // convert back to check if we loss any precision
        let near_value = stake_value * total_staked_near_balance / total_stake_supply;

        (stake_value + (near - near_value)).as_u128().into()
    }

    /// converts STAKE to NEAR rounded down and then adds back the remainder
    /// - the remainder is added back at a 1:1 ratio because i yoctoNEAR is the smallest unit and is
    ///   not further divisible
    pub fn stake_to_near(&self, stake: YoctoStake) -> YoctoNear {
        if self.total_staked_near_balance.value() == 0 || self.total_stake_supply.value() == 0
            // when deposit and staked with staking pool, there is a small amount remaining as unstaked
            // however, STAKE token value should never be less than 1:1 in terms of NEAR
            || self.total_staked_near_balance.value() < self.total_stake_supply.value()
        {
            return stake.value().into();
        }

        let stake = U256::from(stake);
        let total_stake_supply = U256::from(self.total_stake_supply);
        let total_staked_near_balance = U256::from(self.total_staked_near_balance);

        let near_value = stake * total_staked_near_balance / total_stake_supply;

        // convert back to check if we loss any precision
        let stake_value = near_value * total_stake_supply / total_staked_near_balance;

        (near_value + (stake - stake_value)).as_u128().into()
    }

    pub fn log_near_event(&self) {
        log(events::StakeTokenValue::from(*self));
    }
}

impl From<interface::StakeTokenValue> for StakeTokenValue {
    fn from(value: interface::StakeTokenValue) -> Self {
        Self {
            block_time_height: value.block_time_height.into(),
            total_staked_near_balance: value.total_staked_near_balance.into(),
            total_stake_supply: value.total_stake_supply.into(),
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};
    use primitive_types::U512;

    #[test]
    fn when_total_stake_supply_is_zero() {
        let account_id = "bob.near";
        let context = new_context(account_id);
        testing_env!(context);

        let stake_token_value = StakeTokenValue::default();
        assert_eq!(
            stake_token_value.stake_to_near(YOCTO.into()),
            YoctoNear(YOCTO)
        );
        assert_eq!(
            stake_token_value.near_to_stake(YOCTO.into()),
            YoctoStake(YOCTO)
        );

        assert_eq!(
            stake_token_value.stake_to_near((10 * YOCTO).into()),
            YoctoNear(10 * YOCTO)
        );
        assert_eq!(
            stake_token_value.near_to_stake((10 * YOCTO).into()),
            YoctoStake(10 * YOCTO)
        );
    }

    #[test]
    fn ctake_near_conversion() {
        let total_staked_near_balance: u128 = 17206799984076953573143542;
        let total_stake_supply: u128 = 16742879620291694593306687;
        // let stake_value: u128 = 1027708516952066370722277;

        let value = (U512::from(YOCTO) * U512::from(total_staked_near_balance))
            / U512::from(total_stake_supply);
        let remainder = (U512::from(YOCTO) * U512::from(total_staked_near_balance))
            % U512::from(total_stake_supply);

        println!("{} remainder = {}", value, remainder);

        let value = (U256::from(YOCTO) * U256::from(total_staked_near_balance))
            / U256::from(total_stake_supply);
        let remainder = (U256::from(YOCTO) * U256::from(total_staked_near_balance))
            % U256::from(total_stake_supply);

        let amount: u128 = (value * U256::from(total_stake_supply)
            / U256::from(total_staked_near_balance))
        .as_u128()
        .into();

        println!(
            "{} remainder = {}, reverse = {} difference = {}",
            value,
            remainder,
            amount,
            YOCTO - amount
        );

        let remainder = YOCTO - amount;
        let value = value + remainder;

        println!("{}", value);

        let amount: u128 = (value * U256::from(total_stake_supply)
            / U256::from(total_staked_near_balance))
        .as_u128()
        .into();

        println!(
            "{} remainder = {}, reverse = {} difference = {}",
            value,
            remainder,
            amount,
            YOCTO - amount
        );
    }
}
