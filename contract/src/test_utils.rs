pub mod near {
    use near_sdk::{AccountId, VMContext};

    pub fn to_account_id(id: &str) -> AccountId {
        id.to_string()
    }

    pub fn stake_contract_account_id() -> AccountId {
        to_account_id("stake.oysterpack.near")
    }

    pub fn new_context(predecessor_account_id: AccountId) -> VMContext {
        VMContext {
            current_account_id: stake_contract_account_id(),
            signer_account_id: predecessor_account_id.clone(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id,
            input: vec![],
            epoch_height: 0,
            block_index: 0,
            block_timestamp: 0,
            account_balance: 0,
            account_locked_balance: 0,
            storage_usage: 10u64.pow(6),
            attached_deposit: 0,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view: false,
            output_data_receivers: vec![],
        }
    }
}
