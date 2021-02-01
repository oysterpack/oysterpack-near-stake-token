use crate::*;
use crate::{
    core::Hash,
    domain::YoctoStake,
    interface::{FungibleToken, Memo, ResolveTransferCall, TokenAmount, TransferCallMessage},
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
    #[payable]
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
        self.claim_receipt_funds(&mut sender);
        sender.apply_stake_debit(stake_amount);
        // apply the 1 yoctoNEAR that was attached to the sender account's NEAR balance
        sender.apply_near_credit(1.into());

        let mut receiver = self.registered_account(receiver_id.as_ref());
        receiver.apply_stake_credit(stake_amount);

        self.save_registered_account(&sender);
        self.save_registered_account(&receiver);
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
        msg: TransferCallMessage,
        _memo: Option<Memo>,
    ) -> Promise {
        self.ft_transfer(receiver_id.clone(), amount.clone(), _memo);

        ext_transfer_receiver::ft_on_transfer(
            env::predecessor_account_id(),
            amount.clone(),
            msg,
            receiver_id.as_ref(),
            NO_DEPOSIT.value(),
            self.ft_on_transfer_gas(),
        )
        .then(ext_resolve_transfer_call::ft_resolve_transfer_call(
            env::predecessor_account_id(),
            receiver_id.as_ref().to_string(),
            amount,
            &env::current_account_id(),
            NO_DEPOSIT.value(),
            self.resolve_transfer_gas(),
        ))
    }

    fn ft_total_supply(&self) -> TokenAmount {
        self.total_stake.amount().value().into()
    }

    fn ft_balance_of(&self, account_id: ValidAccountId) -> TokenAmount {
        self.accounts
            .get(&Hash::from(account_id))
            .map_or_else(TokenAmount::default, |account| {
                let account = self.apply_receipt_funds_for_view(&account);
                account.stake.map_or_else(TokenAmount::default, |balance| {
                    balance.amount().value().into()
                })
            })
    }
}

impl StakeTokenContract {
    fn resolve_transfer_gas(&self) -> u64 {
        self.config
            .gas_config()
            .callbacks()
            .resolve_transfer_gas()
            .value()
    }

    // pass along remainder of prepaid  gas to receiver contract
    fn ft_on_transfer_gas(&self) -> u64 {
        env::prepaid_gas()
            - env::used_gas()
            - self.resolve_transfer_gas()
            // ft_on_transfer
            - self.config.gas_config().function_call_promise().value()
            // ft_resolve_transfer_call
            - self.config.gas_config().function_call_promise().value()
            // ft_resolve_transfer_call data dependency
            - self
            .config
            .gas_config()
            .function_call_promise_data_dependency()
            .value()
    }

    /// the unused amount is retrieved from the `TransferReceiver::ft_on_transfer` promise result
    fn transfer_call_receiver_unused_amount(&self, transfer_amount: TokenAmount) -> TokenAmount {
        let unused_amount: TokenAmount = match self.promise_result(0) {
            PromiseResult::Successful(result) => {
                serde_json::from_slice(&result).expect("unused token amount")
            }
            _ => {
                log!(
                    "ERROR: transfer call failed on receiver contract - full transfer amount will be refunded"
                );
                transfer_amount.clone()
            }
        };

        if unused_amount.value() > transfer_amount.value() {
            log!(
                "WARNING: unused_amount({}) > amount({}) - full transfer amount will be refunded",
                unused_amount,
                transfer_amount
            );
            transfer_amount
        } else {
            unused_amount
        }
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
        let unused_amount = self.transfer_call_receiver_unused_amount(amount);

        let refund_amount = if unused_amount.value() > 0 {
            log!("unused amount: {}", unused_amount);
            let mut sender = self.registered_account(sender_id.as_ref());
            let mut receiver = self.registered_account(receiver_id.as_ref());
            match receiver.stake.as_mut() {
                Some(balance) => {
                    let refund_amount = if balance.amount().value() < unused_amount.value() {
                        log!("ERROR: partial amount will be refunded because receiver STAKE balance is insufficient");
                        balance.amount()
                    } else {
                        unused_amount.value().into()
                    };
                    receiver.apply_stake_debit(refund_amount);
                    sender.apply_stake_credit(refund_amount);

                    self.save_registered_account(&receiver);
                    self.save_registered_account(&sender);
                    log!("sender refunded: {}", refund_amount.value());
                    refund_amount.value().into()
                }
                None => {
                    log!("ERROR: refund is not possible because receiver STAKE balance is zero");
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

#[ext_contract(ext_transfer_receiver)]
pub trait ExtTransferReceiver {
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

#[cfg(test)]
mod test_transfer {

    use super::*;
    use crate::interface::AccountManagement;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    pub fn transfer_ok() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        assert!(contract.account_registered(to_valid_account_id(sender_id)));
        assert!(contract.account_registered(to_valid_account_id(receiver_id)));

        assert_eq!(contract.ft_total_supply(), 0.into());
        assert_eq!(
            contract.ft_balance_of(to_valid_account_id(sender_id)),
            0.into()
        );
        assert_eq!(
            contract.ft_balance_of(to_valid_account_id(receiver_id)),
            0.into()
        );

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1; // 1 yoctoNEAR is required to transfer
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        contract.ft_transfer(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            None,
        );

        // Assert
        assert_eq!(contract.ft_total_supply().value(), total_supply.value());
        assert_eq!(
            contract
                .ft_balance_of(to_valid_account_id(sender_id))
                .value(),
            total_supply.value() - transfer_amount
        );
        assert_eq!(
            contract
                .ft_balance_of(to_valid_account_id(receiver_id))
                .value(),
            transfer_amount
        );
        let sender = contract.predecessor_registered_account();
        assert_eq!(sender.near.unwrap().amount().value(), 1,
                   "expected the attached 1 yoctoNEAR for the transfer to be credited to the account's NEAR balance");

        // Act - transfer with memo
        testing_env!(context.clone());
        contract.ft_transfer(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            Some("memo".into()),
        );
        let sender = contract.predecessor_registered_account();
        assert_eq!(sender.near.unwrap().amount().value(), 2,
                   "expected the attached 1 yoctoNEAR for the transfer to be credited to the account's NEAR balance");

        // Assert
        assert_eq!(contract.ft_total_supply().value(), total_supply.value());
        assert_eq!(
            contract
                .ft_balance_of(to_valid_account_id(sender_id))
                .value(),
            total_supply.value() - (transfer_amount * 2)
        );
        assert_eq!(
            contract
                .ft_balance_of(to_valid_account_id(receiver_id))
                .value(),
            transfer_amount * 2
        );
    }

    #[test]
    #[should_panic(expected = "account is not registered: sender.near")]
    fn sender_not_registered() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = "sender.near"; // not registered
        let receiver_id = test_ctx.account_id; // registered

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1; // 1 yoctoNEAR is required to transfer
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        contract.ft_transfer(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "account is not registered: receiver.near")]
    fn receiver_not_registered() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id; // registered
        let receiver_id = "receiver.near"; // registered

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1; // 1 yoctoNEAR is required to transfer
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        contract.ft_transfer(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "exactly 1 yoctoNEAR must be attached")]
    pub fn zero_yocto_near_attached() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        contract.ft_transfer(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "exactly 1 yoctoNEAR must be attached")]
    pub fn two_yocto_near_attached() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 2;
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        contract.ft_transfer(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "amount must not be zero")]
    pub fn zero_transfer_amount() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1;
        testing_env!(context.clone());
        let transfer_amount = 0;
        contract.ft_transfer(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "account STAKE balance is too low to fulfill request")]
    pub fn sender_balance_with_insufficient_funds() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(1 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1;
        testing_env!(context.clone());
        let transfer_amount = 2 * YOCTO;
        contract.ft_transfer(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            None,
        );
    }
}

#[cfg(test)]
mod test_transfer_call {
    use super::*;
    use crate::domain::TGAS;
    use crate::interface::AccountManagement;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{serde::Deserialize, serde_json, testing_env, MockedBlockchain};

    #[test]
    pub fn transfer_ok() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        assert!(contract.account_registered(to_valid_account_id(sender_id)));
        assert!(contract.account_registered(to_valid_account_id(receiver_id)));

        assert_eq!(contract.ft_total_supply(), 0.into());
        assert_eq!(
            contract.ft_balance_of(to_valid_account_id(sender_id)),
            0.into()
        );
        assert_eq!(
            contract.ft_balance_of(to_valid_account_id(receiver_id)),
            0.into()
        );

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1; // 1 yoctoNEAR is required to transfer
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        let msg = TransferCallMessage::from("pay");
        contract.ft_transfer_call(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            msg.clone(),
            None,
        );

        // Assert
        assert_eq!(contract.ft_total_supply().value(), total_supply.value());
        assert_eq!(
            contract
                .ft_balance_of(to_valid_account_id(sender_id))
                .value(),
            total_supply.value() - transfer_amount
        );
        assert_eq!(
            contract
                .ft_balance_of(to_valid_account_id(receiver_id))
                .value(),
            transfer_amount
        );
        let sender = contract.predecessor_registered_account();
        assert_eq!(sender.near.unwrap().amount().value(), 1,
                   "expected the attached 1 yoctoNEAR for the transfer to be credited to the account's NEAR balance");

        let receipts = deserialize_receipts();
        assert_eq!(receipts.len(), 2);
        {
            let receipt = &receipts[0];
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name,
                    args,
                    deposit,
                    gas,
                } => {
                    assert_eq!(method_name, "ft_on_transfer");
                    assert_eq!(*deposit, 0);
                    let args: TransferCallArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(args.sender_id, to_valid_account_id(sender_id));
                    assert_eq!(args.amount, transfer_amount.into());
                    assert_eq!(args.msg, msg);
                    assert!(*gas >= context.prepaid_gas - (TGAS * 35).value())
                }
                _ => panic!("expected `ft_on_transfer` function call"),
            }
        }
        {
            let receipt = &receipts[1];
            match &receipt.actions[0] {
                Action::FunctionCall {
                    method_name,
                    args,
                    deposit,
                    gas,
                } => {
                    assert_eq!(method_name, "ft_resolve_transfer_call");
                    assert_eq!(*deposit, 0);
                    let args: ResolveTransferCallArgs = serde_json::from_str(args).unwrap();
                    assert_eq!(args.sender_id, to_valid_account_id(sender_id));
                    assert_eq!(args.receiver_id, to_valid_account_id(receiver_id));
                    assert_eq!(args.amount, transfer_amount.into());
                    assert_eq!(
                        *gas,
                        contract
                            .config
                            .gas_config()
                            .callbacks()
                            .resolve_transfer_gas()
                            .value()
                    )
                }
                _ => panic!("expected `ft_on_transfer` function call"),
            }
        }

        // Act - transfer with memo
        testing_env!(context.clone());
        contract.ft_transfer_call(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            "pay".into(),
            Some("memo".into()),
        );
        let sender = contract.predecessor_registered_account();
        assert_eq!(sender.near.unwrap().amount().value(), 2,
                   "expected the attached 1 yoctoNEAR for the transfer to be credited to the account's NEAR balance");

        // Assert
        assert_eq!(contract.ft_total_supply().value(), total_supply.value());
        assert_eq!(
            contract
                .ft_balance_of(to_valid_account_id(sender_id))
                .value(),
            total_supply.value() - (transfer_amount * 2)
        );
        assert_eq!(
            contract
                .ft_balance_of(to_valid_account_id(receiver_id))
                .value(),
            transfer_amount * 2
        );
    }

    #[test]
    #[should_panic(expected = "account is not registered: sender.near")]
    fn sender_not_registered() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = "sender.near"; // not registered
        let receiver_id = test_ctx.account_id; // registered

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1; // 1 yoctoNEAR is required to transfer
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        contract.ft_transfer_call(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            "pay".into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "account is not registered: receiver.near")]
    fn receiver_not_registered() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id; // registered
        let receiver_id = "receiver.near"; // registered

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1; // 1 yoctoNEAR is required to transfer
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        contract.ft_transfer_call(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            "pay".into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "exactly 1 yoctoNEAR must be attached")]
    pub fn zero_yocto_near_attached() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        contract.ft_transfer_call(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            "pay".into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "exactly 1 yoctoNEAR must be attached")]
    pub fn two_yocto_near_attached() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 2;
        testing_env!(context.clone());
        let transfer_amount = 10 * YOCTO;
        contract.ft_transfer_call(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            "pay".into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "amount must not be zero")]
    pub fn zero_transfer_amount() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(100 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1;
        testing_env!(context.clone());
        let transfer_amount = 0;
        contract.ft_transfer_call(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            "pay".into(),
            None,
        );
    }

    #[test]
    #[should_panic(expected = "account STAKE balance is too low to fulfill request")]
    pub fn sender_balance_with_insufficient_funds() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the sender with STAKE
        let mut sender = contract.registered_account(sender_id);
        let total_supply = YoctoStake(1 * YOCTO);
        sender.apply_stake_credit(total_supply);
        contract.total_stake.credit(total_supply);
        contract.save_registered_account(&sender);

        // Act - transfer with no memo
        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = sender_id.to_string();
        context.attached_deposit = 1;
        testing_env!(context.clone());
        let transfer_amount = 2 * YOCTO;
        contract.ft_transfer_call(
            to_valid_account_id(receiver_id),
            transfer_amount.into(),
            "pay".into(),
            None,
        );
    }

    #[derive(Deserialize, Debug)]
    #[serde(crate = "near_sdk::serde")]
    struct TransferCallArgs {
        sender_id: ValidAccountId,
        amount: TokenAmount,
        msg: TransferCallMessage,
    }

    #[derive(Deserialize, Debug)]
    #[serde(crate = "near_sdk::serde")]
    struct ResolveTransferCallArgs {
        sender_id: ValidAccountId,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
    }
}

#[cfg(test)]
mod test_resolve_transfer_call {
    use super::*;
    use crate::interface::AccountManagement;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{serde_json, testing_env, MockedBlockchain};

    #[test]
    fn err_receiver_has_balance_for_full_refund() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = receiver_id.to_string();

        // register receiver account and credit STAKE
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context.clone());
            contract.register_account();

            context.attached_deposit = 0;
            testing_env!(context.clone());

            let mut receiver = contract.predecessor_registered_account();
            receiver.apply_stake_credit(YOCTO.into());
            contract.save_registered_account(&receiver);
        }

        set_env_with_promise_result(contract, promise_result_failed);

        // Act
        let result = contract.ft_resolve_transfer_call(
            to_valid_account_id(sender_id),
            to_valid_account_id(receiver_id),
            YOCTO.into(),
        );

        // Assert - full amount is refunded
        match result {
            PromiseOrValue::Value(refund_amount) => {
                assert_eq!(refund_amount.value(), YOCTO.into());
                let receiver = contract.registered_account(receiver_id);
                assert!(receiver.stake.is_none());
                let sender = contract.registered_account(sender_id);
                assert_eq!(sender.stake.unwrap().amount(), YOCTO.into());
            }
            _ => panic!("expected value to be returned"),
        }
    }

    #[test]
    fn err_receiver_has_balance_for_partial_refund() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = receiver_id.to_string();

        // register receiver account and credit STAKE
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context.clone());
            contract.register_account();

            context.attached_deposit = 0;
            testing_env!(context.clone());

            let mut receiver = contract.predecessor_registered_account();
            receiver.apply_stake_credit(YOCTO.into());
            contract.save_registered_account(&receiver);
        }

        set_env_with_promise_result(contract, promise_result_failed);

        // Act
        let result = contract.ft_resolve_transfer_call(
            to_valid_account_id(sender_id),
            to_valid_account_id(receiver_id),
            (2 * YOCTO).into(),
        );

        // Assert - partial amount is refunded
        match result {
            PromiseOrValue::Value(refund_amount) => {
                assert_eq!(refund_amount.value(), YOCTO.into());
                let receiver = contract.registered_account(receiver_id);
                assert!(receiver.stake.is_none());
                let sender = contract.registered_account(sender_id);
                assert_eq!(sender.stake.unwrap().amount(), YOCTO.into());
            }
            _ => panic!("expected value to be returned"),
        }
    }

    #[test]
    fn err_receiver_has_zero_balance() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        let mut context = test_ctx.context.clone();
        context.predecessor_account_id = receiver_id.to_string();

        // register receiver account and credit STAKE
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context.clone());
            contract.register_account();

            context.attached_deposit = 0;
            testing_env!(context.clone());
        }

        set_env_with_promise_result(contract, promise_result_failed);

        // Act
        let result = contract.ft_resolve_transfer_call(
            to_valid_account_id(sender_id),
            to_valid_account_id(receiver_id),
            (2 * YOCTO).into(),
        );

        // Assert - full amount is refunded
        match result {
            PromiseOrValue::Value(refund_amount) => {
                assert_eq!(refund_amount.value(), 0);
            }
            _ => panic!("expected value to be returned"),
        }
    }

    #[test]
    pub fn ok_zero_refund() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        set_env_with_promise_result(contract, promise_result_zero_refund);

        // Act
        let result = contract.ft_resolve_transfer_call(
            to_valid_account_id(sender_id),
            to_valid_account_id(receiver_id),
            YOCTO.into(),
        );

        // Assert
        match result {
            PromiseOrValue::Value(refund_amount) => assert_eq!(refund_amount.value(), 0),
            _ => panic!("expected value to be returned"),
        }
    }

    #[test]
    pub fn ok_with_refund() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the receiver with STAKE
        let mut receiver = contract.registered_account(receiver_id);
        receiver.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&receiver);

        set_env_with_promise_result(contract, promise_result_with_refund);

        // Act
        let result = contract.ft_resolve_transfer_call(
            to_valid_account_id(sender_id),
            to_valid_account_id(receiver_id),
            YOCTO.into(),
        );

        // Assert
        match result {
            PromiseOrValue::Value(refund_amount) => assert_eq!(refund_amount.value(), YOCTO),
            _ => panic!("expected value to be returned"),
        }

        assert_eq!(
            contract
                .registered_account(receiver_id)
                .stake
                .unwrap()
                .amount(),
            (99 * YOCTO).into()
        );
        assert_eq!(
            contract
                .registered_account(sender_id)
                .stake
                .unwrap()
                .amount(),
            YOCTO.into()
        );
    }

    #[test]
    pub fn ok_with_refund_and_receiver_zero_balance() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        set_env_with_promise_result(contract, promise_result_with_refund);

        // Act
        let result = contract.ft_resolve_transfer_call(
            to_valid_account_id(sender_id),
            to_valid_account_id(receiver_id),
            YOCTO.into(),
        );

        // Assert
        match result {
            PromiseOrValue::Value(refund_amount) => assert_eq!(refund_amount.value(), 0),
            _ => panic!("expected value to be returned"),
        }
    }

    #[test]
    pub fn ok_with_refund_and_receiver_balance_insufficient() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the receiver with STAKE
        let mut receiver = contract.registered_account(receiver_id);
        receiver.apply_stake_credit((YOCTO / 10).into());
        contract.save_registered_account(&receiver);

        set_env_with_promise_result(contract, promise_result_with_refund);

        // Act
        let result = contract.ft_resolve_transfer_call(
            to_valid_account_id(sender_id),
            to_valid_account_id(receiver_id),
            YOCTO.into(),
        );

        // Assert
        match result {
            PromiseOrValue::Value(refund_amount) => assert_eq!(refund_amount.value(), (YOCTO / 10)),
            _ => panic!("expected value to be returned"),
        }
    }

    #[test]
    pub fn ok_with_refund_gt_transfer_amount() {
        // Arrange
        let mut test_ctx = TestContext::with_registered_account();
        let contract = &mut test_ctx.contract;

        let sender_id = test_ctx.account_id;
        let receiver_id = "receiver.near";

        // register receiver account
        {
            let mut context = test_ctx.context.clone();
            context.predecessor_account_id = receiver_id.to_string();
            context.attached_deposit = YOCTO;
            testing_env!(context);
            contract.register_account();
        }

        // credit the receiver with STAKE
        let mut receiver = contract.registered_account(receiver_id);
        receiver.apply_stake_credit((100 * YOCTO).into());
        contract.save_registered_account(&receiver);

        set_env_with_promise_result(contract, promise_result_with_overrefund);

        // Act
        let result = contract.ft_resolve_transfer_call(
            to_valid_account_id(sender_id),
            to_valid_account_id(receiver_id),
            YOCTO.into(),
        );

        // Assert
        match result {
            PromiseOrValue::Value(refund_amount) => assert_eq!(refund_amount.value(), YOCTO),
            _ => panic!("expected value to be returned"),
        }

        assert_eq!(
            contract
                .registered_account(receiver_id)
                .stake
                .unwrap()
                .amount(),
            (99 * YOCTO).into()
        );
        assert_eq!(
            contract
                .registered_account(sender_id)
                .stake
                .unwrap()
                .amount(),
            YOCTO.into()
        );
    }

    fn promise_result_zero_refund(_result_index: u64) -> PromiseResult {
        PromiseResult::Successful(serde_json::to_vec(&TokenAmount::from(0)).unwrap())
    }

    fn promise_result_with_refund(_result_index: u64) -> PromiseResult {
        PromiseResult::Successful(serde_json::to_vec(&TokenAmount::from(YOCTO)).unwrap())
    }

    fn promise_result_with_overrefund(_result_index: u64) -> PromiseResult {
        PromiseResult::Successful(serde_json::to_vec(&TokenAmount::from(2 * YOCTO)).unwrap())
    }

    fn promise_result_failed(_result_index: u64) -> PromiseResult {
        PromiseResult::Failed
    }
}
