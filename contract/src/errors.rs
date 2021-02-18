//! centralizes all error messages

pub mod asserts {
    pub const PREDECESSOR_MUST_NE_SELF_OR_OPERATOR: &str =
        "contract call is only allowed internally or by an operator account";
    pub const PREDECESSOR_MUST_BE_OPERATOR: &str =
        "contract call is only allowed by an operator account";
    pub const OPERATOR_ID_MUST_NOT_BE_CONTRACT_ID: &str =
        "operator account ID must not be the contract account ID";
    pub const PREDECESSOR_MUST_BE_OWNER: &str =
        "contract call is only allowed by the contract owner";
    pub const ATTACHED_DEPOSIT_IS_REQUIRED: &str = "attached deposit is required";
}

pub mod staking_pool_failures {

    pub const UNSTAKE_FAILURE: &str = "failed to unstake NEAR with staking pool";

    pub const GET_ACCOUNT_FAILURE: &str = "failed to get account info from staking pool";

    pub const WITHDRAW_ALL_FAILURE: &str =
        "failed to withdraw all unstaked funds from staking pool";
}

pub mod staking_errors {
    pub const BLOCKED_BY_BATCH_RUNNING: &str = "action is blocked because a batch is running";

    pub const BLOCKED_BY_STAKE_TOKEN_VALUE_REFRESH: &str =
        "action is blocked because STAKE token value is being refreshed";

    pub const NO_FUNDS_IN_STAKE_BATCH_TO_WITHDRAW: &str = "there are no funds in stake batch";
}

pub mod redeeming_stake_errors {
    pub const NO_REDEEM_STAKE_BATCH_TO_RUN: &str = "there is no redeem stake batch";

    pub const UNSTAKING_BLOCKED_BY_PENDING_WITHDRAWAL: &str =
        "unstaking is blocked until all unstaked NEAR can be withdrawn";

    pub const UNSTAKED_FUNDS_NOT_AVAILABLE_FOR_WITHDRAWAL: &str =
        "unstaked NEAR funds are not yet available for withdrawal";
}

pub mod staking_service {
    pub const DEPOSIT_REQUIRED_FOR_STAKE: &str = "deposit is required in order to stake";

    pub const ZERO_REDEEM_AMOUNT: &str = "redeem amount must not be zero";

    pub const INSUFFICIENT_STAKE_FOR_REDEEM_REQUEST: &str =
        "account STAKE balance is insufficient to fulfill request";

    pub const BATCH_BALANCE_INSUFFICIENT: &str = "batch balance is insufficient to fulfill request";
}

pub mod illegal_state {
    pub const STAKE_BATCH_SHOULD_EXIST: &str = "ILLEGAL STATE : stake batch should exist";

    pub const REDEEM_STAKE_BATCH_SHOULD_EXIST: &str =
        "ILLEGAL STATE : redeem stake batch should exist";

    pub const REDEEM_STAKE_BATCH_RECEIPT_SHOULD_EXIST: &str =
        "ILLEGAL STATE : redeem stake batch receipt should exist";

    pub const ILLEGAL_REDEEM_LOCK_STATE: &str = "ILLEGAL STATE : illegal redeem lock state";
}

pub mod account_management {
    pub const INSUFFICIENT_STORAGE_FEE: &str =
        "sufficient deposit is required to pay for account storage fees";

    pub const ACCOUNT_ALREADY_REGISTERED: &str = "account is already registered";

    pub const UNREGISTER_REQUIRES_ZERO_BALANCES: &str =
        "all funds must be withdrawn from the account in order to unregister";

    pub const ACCOUNT_NOT_REGISTERED: &str = "account is not registered";
}

pub mod contract_owner {

    pub const INSUFFICIENT_FUNDS_FOR_OWNER_WITHDRAWAL: &str =
        "owner balance is too low to fulfill withdrawal request";

    pub const INSUFFICIENT_FUNDS_FOR_OWNER_STAKING: &str =
        "owner balance is too low to fulfill stake request";

    pub const TRANSFER_TO_NON_REGISTERED_ACCOUNT: &str =
        "contract ownership can only be transferred to a registered account";
}
