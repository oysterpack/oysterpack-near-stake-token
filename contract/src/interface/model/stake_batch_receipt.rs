use crate::interface::YoctoStake;
use crate::{
    domain,
    interface::{StakeTokenValue, YoctoNear},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeBatchReceipt {
    /// tracks amount of NEAR that has been claimed on the receipt
    /// - when the amount reaches zero, then the receipt is deleted
    pub staked_near: YoctoNear,

    pub stake_minted: YoctoStake,

    /// the STAKE token value at the point in time when the batch was run
    /// - is used to compute the amount of STAKE tokens to issue to the account based on the amount
    ///   of NEAR that was staked
    pub stake_token_value: StakeTokenValue,
}

impl From<domain::StakeBatchReceipt> for StakeBatchReceipt {
    fn from(receipt: domain::StakeBatchReceipt) -> Self {
        Self {
            staked_near: receipt.staked_near().into(),
            stake_token_value: receipt.stake_token_value().into(),
            stake_minted: receipt
                .stake_token_value()
                .near_to_stake(receipt.staked_near())
                .into(),
        }
    }
}
