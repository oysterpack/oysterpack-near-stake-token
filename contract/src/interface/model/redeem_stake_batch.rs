use crate::interface::RedeemStakeBatchReceipt;
use crate::{
    domain,
    interface::{BatchId, TimestampedStakeBalance, YoctoNear},
};
use near_sdk::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct RedeemStakeBatch {
    pub id: BatchId,
    pub balance: TimestampedStakeBalance,
    /// if receipt is present it means the STAKE has been redeemed and the unstaked NEAR is still locked
    /// by the staking pool for withdrawal
    pub receipt: Option<RedeemStakeBatchReceipt>,
    /// the NEAR value of the redeemed STAKE computed from [stake_token_value](RedeemStakeBatchReceipt::RedeemStakeBatchReceipt)
    pub redeemed_stake_value: Option<YoctoNear>,
}

impl RedeemStakeBatch {
    pub fn from(batch: domain::RedeemStakeBatch, receipt: Option<RedeemStakeBatchReceipt>) -> Self {
        let redeemed_stake_value = receipt.as_ref().map(|receipt| {
            domain::StakeTokenValue::from(receipt.stake_token_value.clone())
                .stake_to_near(batch.balance().amount().into())
                .into()
        });
        Self {
            id: BatchId(batch.id().0.into()),
            balance: batch.balance().into(),
            receipt,
            redeemed_stake_value,
        }
    }
}
