use near_sdk::VMContext;

pub fn ctx() -> VMContext {
    VMContext {
        current_account_id: "".to_string(),
        signer_account_id: "".to_string(),
        signer_account_pk: vec![0, 1, 2],
        predecessor_account_id: "".to_string(),
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