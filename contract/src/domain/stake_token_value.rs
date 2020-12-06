use crate::domain::{
    BlockTimeHeight, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear, YoctoStake,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

/// STAKE token value at a point in time, i.e., at a block height.
///
/// STAKE token value = [total_staked_near_balance] / [total_stake_supply]
///
/// NOTE: The STAKE token value is gathered while the contract is locked.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct StakeTokenValue {
    block_time_height: BlockTimeHeight,
    total_staked_near_balance: TimestampedNearBalance,
    total_stake_supply: TimestampedStakeBalance,
}
