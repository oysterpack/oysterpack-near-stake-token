use crate::domain::{
    BlockTimeHeight, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear, YoctoStake,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct UnstakeBatch {
    redeemed_stake: YoctoStake,
    /// the value of the STAKE tokens that are being redeemed in this batch, which will be unstaked
    unstaked_near: YoctoNear,

    /// STAKE token NEAR value at a point in time, i.e., at a block height
    block_time_height: BlockTimeHeight,
    total_staked_near_balance: TimestampedNearBalance,
    total_stake_supply: TimestampedStakeBalance,
}
