use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::Promise;

pub trait FungibleTokenCore {
    /// #\[payable\]
    fn ft_transfer(&mut self, receiver_id: ValidAccountId, amount: U128, memo: Option<String>);

    /// #\[payable\]
    fn ft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        amount: U128,
        msg: String,
        memo: Option<String>,
    ) -> Promise;

    fn ft_total_supply(&self) -> U128;

    fn ft_balance_of(&self, account_id: ValidAccountId) -> U128;
}

pub trait FungibleTokenCoreResolveTransfer {
    /// #\[private\]
    fn ft_resolve_transfer(
        &mut self,
        sender_id: ValidAccountId,
        receiver_id: ValidAccountId,
        amount: U128,
        // #[callback_result]
        used_amount: CallbackResult<U128>, // NOTE: this interface is not supported yet and has to
                                           // be handled using lower level interface.
    ) -> U128;
}

pub struct CallbackResult<T>(T);
