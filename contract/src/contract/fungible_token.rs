use crate::domain::Gas;
use crate::interface::ResolveTransferCall;
use crate::*;
use crate::{
    core::Hash,
    domain::{YoctoStake, TGAS},
    interface::{FungibleToken, Memo, TokenAmount, TransferCallMessage},
    near::NO_DEPOSIT,
};
use near_sdk::{
    env, ext_contract, json_types::ValidAccountId, log, near_bindgen, serde_json, Promise,
    PromiseResult,
};
#[allow(unused_imports)]
use near_sdk::{AccountId, PromiseOrValue};

#[near_bindgen]
impl FungibleToken for StakeTokenContract {
    fn ft_transfer(
        &mut self,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
        _memo: Option<Memo>,
    ) {
        assert_yocto_near_attached();
        assert_token_amount_not_zero(&amount);

        let stake_amount: YoctoStake = amount.value().into();

        let mut sender = self.predecessor_registered_account();
        sender.apply_stake_debit(stake_amount);
        sender.apply_near_credit(1.into());

        let mut receiver = self.registered_account(receiver_id.as_ref());
        receiver.apply_stake_credit(stake_amount);

        self.save_registered_account(&sender);
        self.save_registered_account(&receiver);
    }

    fn ft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
        msg: TransferCallMessage,
        _memo: Option<Memo>,
    ) -> Promise {
        self.ft_transfer(receiver_id.clone(), amount.clone(), _memo);

        let resolve_transfer_gas: Gas = TGAS * 10;
        let gas = { env::prepaid_gas() - env::used_gas() - resolve_transfer_gas.value() };

        ext_on_transfer::ft_on_transfer(
            env::predecessor_account_id(),
            amount.clone(),
            msg,
            receiver_id.as_ref(),
            NO_DEPOSIT.value(),
            gas,
        )
        .then(ext_resolve_transfer_call::ft_resolve_transfer_call(
            env::predecessor_account_id(),
            receiver_id.as_ref().to_string(),
            amount,
            &env::current_account_id(),
            NO_DEPOSIT.value(),
            resolve_transfer_gas.value(),
        ))
    }

    fn ft_total_supply(&self) -> TokenAmount {
        self.total_stake.amount().value().into()
    }

    fn ft_balance_of(&self, account_id: ValidAccountId) -> TokenAmount {
        self.accounts
            .get(&Hash::from(account_id))
            .map_or_else(TokenAmount::default, |account| {
                account.stake.map_or_else(TokenAmount::default, |balance| {
                    balance.amount().value().into()
                })
            })
    }
}

#[near_bindgen]
impl ResolveTransferCall for StakeTokenContract {
    #[private]
    fn ft_resolve_transfer_call(
        &mut self,
        sender_id: ValidAccountId,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
    ) -> PromiseOrValue<TokenAmount> {
        assert_eq!(
            env::promise_results_count(),
            1,
            "transfer call recipient should have returned unused transfer amount"
        );
        let unused_amount: TokenAmount = match env::promise_result(0) {
            PromiseResult::Successful(result) => {
                serde_json::from_slice(&result).expect("unsued token amount")
            }
            _ => 0.into(),
        };

        let unused_amount = if unused_amount.value() > amount.value() {
            log!(
                "WARNING: unused_amount({}) > amount({}) - refunding full amount back to sender",
                unused_amount,
                amount
            );
            amount
        } else {
            unused_amount
        };

        let refund_amount = if unused_amount.value() > 0 {
            log!("receiver returned unused amount: {}", unused_amount);
            let mut sender = self.registered_account(sender_id.as_ref());
            let mut receiver = self.registered_account(receiver_id.as_ref());
            match receiver.stake.as_mut() {
                Some(balance) => {
                    let refund_amount = if balance.amount().value() < unused_amount.value() {
                        log!("ERROR: partial refund will be applied because receiver STAKE balance is less than specified unused amount");
                        balance.amount()
                    } else {
                        unused_amount.value().into()
                    };
                    receiver.apply_stake_debit(refund_amount);
                    sender.apply_stake_credit(refund_amount);

                    self.save_registered_account(&receiver);
                    self.save_registered_account(&sender);
                    log!("sender has been refunded: {}", refund_amount.value());
                    refund_amount.value().into()
                }
                None => {
                    log!("ERROR: receiver STAKE balance is zero");
                    0.into()
                }
            }
        } else {
            unused_amount
        };
        PromiseOrValue::Value(refund_amount)
    }
}

fn assert_yocto_near_attached() {
    assert_eq!(
        env::attached_deposit(),
        1,
        "exactly 1 yoctoNEAR must be attached"
    )
}

fn assert_token_amount_not_zero(amount: &TokenAmount) {
    assert!(amount.value() > 0, "amount must not be zero")
}

#[ext_contract(ext_on_transfer)]
pub trait ExtOnTransfer {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: TokenAmount,
        msg: TransferCallMessage,
    ) -> PromiseOrValue<TokenAmount>;
}

#[ext_contract(ext_resolve_transfer_call)]
pub trait ExtResolveTransferCall {
    fn ft_resolve_transfer_call(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: TokenAmount,
    ) -> PromiseOrValue<TokenAmount>;
}
