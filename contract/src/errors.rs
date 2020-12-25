pub mod asserts {
    pub const PREDECESSOR_MUST_BE_SELF: &str = "contract call is only allowed internally";
    pub const PREDECESSOR_MUST_NE_SELF_OR_OPERATOR: &str =
        "contract call is only allowed internally or by an operator account";
    pub const PREDECESSOR_MUST_BE_OPERATOR: &str =
        "contract call is only allowed by an operator account";
    pub const OPERATOR_ID_MUST_NOT_BE_CONTRACT_ID: &str =
        "operator account ID must not be the contract account ID";
    pub const PREDECESSOR_MUST_BE_OWNER: &str =
        "contract call is only allowed by the contract owner";
}

pub mod staking_pool_failures {
    pub const GET_STAKED_BALANCE_FAILURE: &str = "failed to get staked balance from staking pool";

    pub const DEPOSIT_AND_STAKE_FAILURE: &str = "failed to deposit and stake into staking pool";

    pub const UNSTAKE_FAILURE: &str = "failed to unstake NEAR with staking pool";

    pub const GET_ACCOUNT_FAILURE: &str = "failed to get account info from staking pool";

    pub const WITHDRAW_ALL_FAILURE: &str =
        "failed to withdraw all unstaked funds from staking pool";
}

pub mod staking_errors {
    pub const BLOCKED_BY_STAKE_BATCH_RUNNING: &str =
        "request is blocked by a running staking batch";

    pub const BLOCKED_BY_UNSTAKING: &str = "request is blocked while unstaking is in progress";

    pub const NO_STAKE_BATCH_TO_RUN: &str = "there is no stake batch to run";

    pub const NO_FUNDS_IN_STAKE_BATCH_TO_WITHDRAW: &str = "there are no funds in stake batch";
}

pub mod redeeming_stake_errors {
    /// redeem stake batch cannot be run while NEAR is being staked
    pub const REDEEM_STAKE_BATCH_BLOCKED_BY_STAKE_BATCH_RUN: &str =
        "RedeemStakeBatch is blocked by StakeBatch run";

    pub const NO_REDEEM_STAKE_BATCH_TO_RUN: &str = "there is no redeem stake batch";

    pub const UNSTAKED_FUNDS_PENDING_WITHDRAWAL: &str =
        "unstaked funds are not yet available for withdrawal";

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
}

pub mod vault_fungible_token {
    pub const RECEIVER_MUST_NOT_BE_SENDER: &str = "receiver account must not be the sender";

    pub const ACCOUNT_INSUFFICIENT_STAKE_FUNDS: &str =
        "account STAKE balance is to low to fulfill request";

    pub const ACCOUNT_INSUFFICIENT_NEAR_FUNDS: &str =
        "account NEAR balance is too low to fulfill request";

    pub const VAULT_DOES_NOT_EXIST: &str = "vault does not exist";

    pub const VAULT_ACCESS_DENIED: &str = "vault access is denied";

    pub const VAULT_INSUFFICIENT_FUNDS: &str =
        "vault balance is too low to fulfill withdrawal request";
}

pub mod contract_owner {
    pub const ACCOUNT_VALIDATION_NEAR_TRANSFER_FAILED: &str =
        "account validation NEAR transfer failed";

    pub const INSUFFICIENT_FUNDS_FOR_OWNER_WITHDRAWAL: &str =
        "owner balance is too low to fulfill withdrawal request";

    pub const INSUFFICIENT_FUNDS_FOR_OWNER_STAKING: &str =
        "owner balance is too low to fulfill stake request";
}
