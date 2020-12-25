use crate::{
    domain,
    interface::{EpochHeight, StakeTokenValue, YoctoStake},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RedeemStakeBatchReceipt {
    /// tracks amount of STAKE that has been claimed on the receipt
    /// - when the amount reaches zero, then the receipt is deleted
    pub redeemed_stake: YoctoStake,

    /// the STAKE token value at the point in time when the batch was run
    /// - is used to compute the amount of STAKE tokens to issue to the account based on the amount
    ///   of NEAR that was staked
    pub stake_token_value: StakeTokenValue,
}

impl RedeemStakeBatchReceipt {
    /// returns true if the unstaked NEAR should be available for withdrawal based on how much time
    /// has passed since the NEAR funds were unstaked.
    /// - current staking pools hold unstaked NEAR funds for 4 epochs before releasing them for withdrawal
    pub fn unstaked_near_withdrawal_availability(&self) -> EpochHeight {
        EpochHeight(
            (self
                .stake_token_value
                .block_time_height
                .epoch_height
                .value()
                + 4)
            .into(),
        )
    }
}

impl From<domain::RedeemStakeBatchReceipt> for RedeemStakeBatchReceipt {
    fn from(receipt: domain::RedeemStakeBatchReceipt) -> Self {
        Self {
            redeemed_stake: receipt.redeemed_stake().into(),
            stake_token_value: receipt.stake_token_value().into(),
        }
    }
}
