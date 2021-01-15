#![allow(dead_code)]

extern crate oysterpack_near_stake_token;
extern crate staking_pool_mock;

use near_sdk_sim::*;

use near_sdk::AccountId;
use near_sdk_sim::errors::TxExecutionError;
use near_sdk_sim::transaction::ExecutionStatus;
use oysterpack_near_stake_token::{near::YOCTO, ContractSettings, StakeTokenContractContract};
use staking_pool_mock::StakingPoolContract;

lazy_static! {
    static ref WASM_BYTES: &'static [u8] =
        include_bytes!("../res/oysterpack_near_stake_token.wasm").as_ref();
    static ref STAKING_POOL_WASM_BYTES: &'static [u8] =
        include_bytes!("../res/staking_pool_mock.wasm").as_ref();
}

pub struct TestContext {
    pub master_account: UserAccount,
    pub contract: ContractAccount<StakeTokenContractContract>,
    pub contract_owner: UserAccount,
    pub contract_account_id: AccountId,
    pub contract_operator: UserAccount,
    pub settings: ContractSettings,
}

impl TestContext {
    pub fn master_account(&self) -> &UserAccount {
        &self.master_account
    }

    pub fn contract_owner(&self) -> &UserAccount {
        &self.contract_owner
    }

    pub fn contract_operator(&self) -> &UserAccount {
        &self.contract_operator
    }

    pub fn contract(&self) -> &ContractAccount<StakeTokenContractContract> {
        &self.contract
    }

    pub fn contract_account_id(&self) -> &str {
        &self.contract_account_id
    }

    pub fn settings(&self) -> &ContractSettings {
        &self.settings
    }
}

pub fn create_context() -> TestContext {
    let master_account = init_simulator(None);
    let contract_owner = master_account.create_user("oysterpack".to_string(), 1000 * YOCTO);
    let contract_operator = contract_owner.create_user("operator".to_string(), 10 * YOCTO);

    let settings = ContractSettings::new(
        "astro-stakers-poolv1".to_string(),
        contract_operator.account_id(),
        None,
    );

    let contract = deploy!(
        // Contract Proxy
        contract: StakeTokenContractContract,
        // Contract account id
        contract_id: "astro-stakers-poolv1-stake-oysterpack",
        // Bytes of contract
        bytes: &WASM_BYTES,
        // User deploying the contract,
        signer_account: master_account,
        // init method
        init_method: new(None, settings.clone())
    );
    let contract_account_id = contract.user_account.account_id();

    // deploy staking pool contract mock
    deploy!(
        // Contract Proxy
        contract: StakingPoolContract,
        // Contract account id
        contract_id: "astro-stakers-poolv1",
        // Bytes of contract
        bytes: &STAKING_POOL_WASM_BYTES,
        // User deploying the contract,
        signer_account: master_account,
        // init method
        init_method: new()
    );

    TestContext {
        master_account,
        contract,
        contract_account_id,
        contract_owner,
        contract_operator,
        settings,
    }
}

pub fn assert_private_func_call(result: ExecutionResult, func_name: &str) {
    if let ExecutionStatus::Failure(TxExecutionError::ActionError(err)) = result.status() {
        assert!(err
            .to_string()
            .contains(&format!("Method {} is private", func_name)));
    } else {
        panic!("expected failure");
    }
}
