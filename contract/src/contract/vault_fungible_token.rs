#![allow(unused_imports)]

use crate::interface::VaultId;
use crate::{
    core::Hash,
    domain::{self, Account, RedeemLock, RedeemStakeBatch, StakeBatch},
    interface::{
        BatchId, RedeemStakeBatchReceipt, StakeTokenValue, StakingService, VaultFungibleToken,
        YoctoNear, YoctoStake,
    },
    near::NO_DEPOSIT,
    StakeTokenContract,
};
use near_sdk::json_types::ValidAccountId;
use near_sdk::{
    env, ext_contract,
    json_types::U128,
    near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId, Promise,
};

#[near_bindgen]
impl VaultFungibleToken for StakeTokenContract {
    fn transfer(&mut self, receiver_id: ValidAccountId, amount: YoctoStake) {
        unimplemented!()
    }

    fn transfer_with_vault(
        &mut self,
        receiver_id: ValidAccountId,
        amount: YoctoStake,
        payload: String,
    ) -> Promise {
        unimplemented!()
    }

    fn withdraw_from_vault(
        &mut self,
        vault_id: VaultId,
        receiver_id: ValidAccountId,
        amount: YoctoStake,
    ) {
        unimplemented!()
    }

    fn get_total_supply(&self) -> YoctoStake {
        unimplemented!()
    }

    fn get_balance(&self, account_id: ValidAccountId) -> YoctoStake {
        unimplemented!()
    }
}

#[near_bindgen]
impl StakeTokenContract {
    pub fn resolve_vault(&mut self, vault_id: VaultId, sender_id: AccountId) -> YoctoStake {
        unimplemented!()
    }
}
