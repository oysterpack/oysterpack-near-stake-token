use crate::domain::{
    BlockTimeHeight, StakeTokenValue, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear,
    YoctoStake,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct RedeemStakeBatchReceipt {
    redeemed_stake: YoctoStake,
    /// the value of the STAKE tokens that are being redeemed in this batch, which will be unstaked
    unstaked_near: YoctoNear,

    stake_token_value: StakeTokenValue,
}
