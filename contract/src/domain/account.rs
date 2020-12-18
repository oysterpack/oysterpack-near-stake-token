use crate::domain::stake_batch::StakeBatch;
use crate::domain::{
    RedeemStakeBatch, StorageUsage, TimestampedNearBalance, TimestampedStakeBalance, YoctoNear,
    YoctoStake,
};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Account {
    /// account is responsible for paying for its own storage fees
    /// the funds are escrowed and refunded when the account is unregistered
    pub storage_escrow: TimestampedNearBalance,

    /// NEAR funds that are available for withdrawal
    pub near: Option<TimestampedNearBalance>,
    /// STAKE tokens that the account owns
    pub stake: Option<TimestampedStakeBalance>,

    /// users will deposit NEAR funds into a batch that will be processed, i.e. deposited and staked
    /// into the staking pool, at scheduled intervals (at least once per epoch)
    /// - STAKE token value is computed when batches are processed in order to issue STAKE tokens
    ///   for NEAR that was staked
    /// - when the account is accessed, the [StakeBatch] status is checked - if processed, then the
    ///   STAKE token value is looked up for the batch and the account is credited with STAKE tokens
    ///   and the batch is cleared
    /// - when funds are claimed, the account is refunded storage fees
    pub stake_batch: Option<StakeBatch>,
    /// if the contract is locked, then deposit the NEAR funds in the next batch
    pub next_stake_batch: Option<StakeBatch>,

    /// when a user wants to redeem STAKE tokens, they are moved from the [stake] balance into the
    /// [redeem_stake_batch] balance.
    /// - STAKE tokens become locked, i.e., they can no longer be traded
    /// - when the account is accessed, the [RedeemStakeBatch] status is checked - if processed, then
    ///   the STAKE token value is looked up for the batch and the account is credited with NEAR token
    ///   and the batch is cleared
    /// - when funds are claimed, the account is refunded storage fees
    pub redeem_stake_batch: Option<RedeemStakeBatch>,
    /// if the contract is locked, then deposit the NEAR funds in the next batch
    pub next_redeem_stake_batch: Option<RedeemStakeBatch>,
}

impl Account {
    pub fn new(storage_escrow_fee: YoctoNear) -> Self {
        Self {
            storage_escrow: TimestampedNearBalance::new(storage_escrow_fee),
            near: None,
            stake: None,
            stake_batch: None,
            next_stake_batch: None,
            redeem_stake_batch: None,
            next_redeem_stake_batch: None,
        }
    }

    /// the purpose for this constructor is to create a fully allocated [Account] object instance
    /// to be used to compute storage usage during the account registration process
    /// - the account is responsible to pay for its storage fees - in order to compute storage fees
    ///   a temporary instance is stored and then the storage usage is measured at runtime
    pub(crate) fn account_template_to_measure_storage_usage() -> Self {
        Self {
            storage_escrow: TimestampedNearBalance::new(0.into()),
            near: Some(TimestampedNearBalance::new(0.into())),
            stake: Some(TimestampedStakeBalance::new(0.into())),
            stake_batch: Some(StakeBatch::new(0.into(), 0.into())),
            next_stake_batch: Some(StakeBatch::new(0.into(), 0.into())),
            redeem_stake_batch: Some(RedeemStakeBatch::new(0.into(), 0.into())),
            next_redeem_stake_batch: Some(RedeemStakeBatch::new(0.into(), 0.into())),
        }
    }

    pub fn has_funds(&self) -> bool {
        self.near.map_or(false, |balance| balance > 0)
            || self.stake.map_or(false, |balance| balance > 0)
            || self.stake_batch.map_or(false, |batch| batch.balance() > 0)
            || self
                .next_stake_batch
                .map_or(false, |batch| batch.balance() > 0)
            || self
                .redeem_stake_batch
                .map_or(false, |batch| batch.balance() > 0)
            || self
                .next_redeem_stake_batch
                .map_or(false, |batch| batch.balance() > 0)
    }

    pub fn apply_near_credit(&mut self, credit: YoctoNear) {
        self.near
            .get_or_insert_with(|| TimestampedNearBalance::new(YoctoNear(0)))
            .credit(credit);
    }

    pub fn apply_near_debit(&mut self, debit: YoctoNear) {
        self.near
            .get_or_insert_with(|| TimestampedNearBalance::new(YoctoNear(0)))
            .debit(debit);
        if let Some(balance) = self.near {
            if balance.amount().value() == 0 {
                self.near = None
            }
        }
    }

    pub fn apply_stake_credit(&mut self, credit: YoctoStake) {
        self.stake
            .get_or_insert_with(|| TimestampedStakeBalance::new(YoctoStake(0)))
            .credit(credit);
    }

    pub fn apply_stake_debit(&mut self, debit: YoctoStake) {
        self.stake
            .get_or_insert_with(|| TimestampedStakeBalance::new(YoctoStake(0)))
            .debit(debit);

        if let Some(balance) = self.stake {
            if balance.amount().value() == 0 {
                self.stake = None
            }
        }
    }
}
