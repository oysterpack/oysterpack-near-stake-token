use crate::{
    domain,
    interface::{StakeTokenValue, YoctoNear, YoctoStake},
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
    /// the NEAR value of the redeemed STAKE computed from [stake_token_value](RedeemStakeBatchReceipt::RedeemStakeBatchReceipt)
    pub redeemed_stake_value: YoctoNear,
}

impl From<domain::RedeemStakeBatchReceipt> for RedeemStakeBatchReceipt {
    fn from(receipt: domain::RedeemStakeBatchReceipt) -> Self {
        Self {
            redeemed_stake: receipt.redeemed_stake().into(),
            stake_token_value: receipt.stake_token_value().into(),
            redeemed_stake_value: receipt.stake_near_value().into(),
        }
    }
}
