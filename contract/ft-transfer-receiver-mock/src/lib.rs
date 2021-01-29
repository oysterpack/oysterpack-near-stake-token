use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    json_types::{ValidAccountId, U128},
    log, near_bindgen,
    serde::{Deserialize, Serialize},
    serde_json::{self, json},
    wee_alloc, AccountId, Promise, PromiseOrValue,
};
use std::{
    cmp::Ordering,
    fmt::{self, Display, Formatter},
};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct TransferReceiverMock {}

const TGAS: u64 = 1_000_000_000_000;
const YOCTO: u128 = 1_000_000_000_000_000_000_000_000;

#[near_bindgen]
impl TransferReceiver for TransferReceiverMock {
    fn ft_on_transfer(
        &mut self,
        #[allow(unused_variables)] sender_id: ValidAccountId,
        amount: TokenAmount,
        msg: String,
    ) -> PromiseOrValue<TokenAmount> {
        log!("{:#?}", msg);
        let msg = serde_json::from_str(&msg).expect("invalid msg");
        match msg {
            Message::Panic => panic!("BOOM!"),
            Message::Accept {
                transfer_relay,
                refund_percent,
            } => {
                if let Some(relay) = transfer_relay {
                    let transfer_relay_amount = amount.value() * relay.percent as u128 / 100;
                    self.invoke_ft_transfer(
                        &env::predecessor_account_id(),
                        &relay.account_id,
                        transfer_relay_amount.into(),
                    )
                    .then(self.invoke_resolve_ft_on_transfer(amount, refund_percent))
                    .into()
                } else {
                    let refund_amount = amount.value() * refund_percent as u128 / 100;
                    PromiseOrValue::Value(refund_amount.into())
                }
            }
        }
    }
}

#[near_bindgen]
impl TransferReceiverMock {
    #[private]
    pub fn resolve_ft_on_transfer(&self, amount: TokenAmount, refund_percent: u8) -> TokenAmount {
        let refund_amount = amount.value() * refund_percent as u128 / 100;
        refund_amount.into()
    }

    pub fn register_account(&self, contract_id: ValidAccountId) -> Promise {
        Promise::new(contract_id.as_ref().to_string()).function_call(
            b"register_account".to_vec(),
            vec![],
            YOCTO,
            5 * TGAS,
        )
    }

    pub fn unregister_account(&self, contract_id: ValidAccountId) -> Promise {
        Promise::new(contract_id.as_ref().to_string()).function_call(
            b"unregister_account".to_vec(),
            vec![],
            YOCTO,
            10 * TGAS,
        )
    }

    pub fn ft_transfer(
        &self,
        token_contract: ValidAccountId,
        receiver_id: ValidAccountId,
        amount: TokenAmount,
    ) -> Promise {
        self.invoke_ft_transfer(token_contract.as_ref(), receiver_id.as_ref(), amount)
    }

    fn invoke_ft_transfer(
        &self,
        token_contract: &str,
        receiver_id: &str,
        amount: TokenAmount,
    ) -> Promise {
        Promise::new(token_contract.to_string()).function_call(
            b"ft_transfer".to_vec(),
            json!({
            "receiver_id": receiver_id,
            "amount":amount
            })
            .to_string()
            .into_bytes(),
            1,
            10 * TGAS,
        )
    }

    fn invoke_resolve_ft_on_transfer(&self, amount: TokenAmount, refund_percent: u8) -> Promise {
        Promise::new(env::current_account_id()).function_call(
            b"resolve_ft_on_transfer".to_vec(),
            json!({
            "amount": amount,
            "refund_percent": refund_percent
            })
            .to_string()
            .into_bytes(),
            0,
            5 * TGAS,
        )
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum Message {
    /// used to instruct the receiver to accept the transfer and specifies instructions on how
    /// to handle it to simulate different test scenarios
    Accept {
        /// specifies how much to refund
        /// - over refund can be simulated by specifying a percentage > 100
        refund_percent: u8,
        /// if set, then receiver funds will be transferred over to this account to simulate spending
        /// the received tokens
        transfer_relay: Option<TransferRelay>,
    },
    // used to instruct the receiver to panic to simulate failure snenario
    Panic,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct TransferRelay {
    account_id: AccountId,
    /// specifies percentage of received amount to transfer over
    percent: u8,
}

pub trait TransferReceiver {
    /// Callback to receive tokens.
    ///
    /// Called by fungible token contract `env::predecessor_account_id` after `transfer_call` was initiated by
    /// `sender_id` of the given `amount` with the transfer message given in `msg` field.
    /// The `amount` of tokens were already transferred to this contract account and ready to be used.
    ///
    /// The method must return the amount of tokens that are not used/accepted by this contract from
    /// the transferred amount, e.g.:
    /// - The transferred amount was `500`, the contract completely takes it and must return `0`.
    /// - The transferred amount was `500`, but this transfer call only needs `450` for the action passed in the `msg`
    ///   field, then the method must return `50`.
    /// - The transferred amount was `500`, but the action in `msg` field has expired and the transfer must be
    ///   cancelled. The method must return `500` or panic.
    ///
    /// Arguments:
    /// - `sender_id` - the account ID that initiated the transfer.
    /// - `amount` - the amount of tokens that were transferred to this account.
    /// - `msg` - a string message that was passed with this transfer call.
    ///
    /// Returns the amount of tokens that are used/accepted by this contract from the transferred amount.
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: TokenAmount,
        msg: String,
    ) -> PromiseOrValue<TokenAmount>;
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenAmount(pub U128);

impl From<u128> for TokenAmount {
    fn from(value: u128) -> Self {
        Self(U128::from(value))
    }
}

impl TokenAmount {
    pub fn value(&self) -> u128 {
        self.0 .0
    }
}

impl Display for TokenAmount {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0 .0.fmt(f)
    }
}

impl PartialOrd for TokenAmount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value().partial_cmp(&other.value())
    }
}

impl Default for TokenAmount {
    fn default() -> Self {
        Self(U128(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use near_sdk::serde_json::{self, json};

    #[test]
    fn message() {
        let msg = Message::Accept {
            refund_percent: 0,
            transfer_relay: None,
        };
        let json = serde_json::to_string_pretty(&msg).unwrap();
        println!("{}", json);

        let json = json!({
        "Accept": {
            "refund_percent": 0,
            "transfer_relay": {"account_id": "account.near", "percent": 50}
          }
        });
        let json = serde_json::to_string(&json).unwrap();
        println!("{}", json);
        let msg: Message = serde_json::from_str(&json).unwrap();
        match msg {
            Message::Accept {
                refund_percent,
                transfer_relay,
            } => {
                println!(
                    "refund_percent={}% transfer_relay={:?}",
                    refund_percent, transfer_relay
                )
            }
            Message::Panic => panic!("expected Accept message type"),
        }

        let msg = Message::Panic;
        let json = serde_json::to_string_pretty(&msg).unwrap();
        println!("{}", json);
        let msg: Message = serde_json::from_str(&json).unwrap();
        println!("{:?}", msg);
    }
}
