#![allow(dead_code)]

extern crate oysterpack_near_stake_token;
extern crate staking_pool_mock;

use near_sdk_sim::*;

use crate::account_management_client::AccountManagementClient;
use crate::financials_client::FinancialsClient;
use crate::operator_client::OperatorClient;
use crate::staking_pool_client::StakingPoolClient;
use crate::staking_service_client::StakingServiceClient;
use near_sdk::json_types::ValidAccountId;
use near_sdk::AccountId;
use near_sdk_sim::errors::TxExecutionError;
use near_sdk_sim::{
    runtime::{init_runtime, RuntimeStandalone},
    transaction::ExecutionStatus,
};
use oysterpack_near_stake_token::{
    interface::StakeAccount, near::YOCTO, StakeTokenContractContract,
};
use staking_pool_mock::StakingPoolContract;
use std::{cell::RefCell, collections::HashMap, convert::TryInto, rc::Rc};

lazy_static! {
    static ref WASM_BYTES: &'static [u8] =
        include_bytes!("../res/oysterpack_near_stake_token.wasm").as_ref();
    static ref STAKING_POOL_WASM_BYTES: &'static [u8] =
        include_bytes!("../res/staking_pool_mock.wasm").as_ref();
}

pub struct TestContext {
    pub runtime: Rc<RefCell<RuntimeStandalone>>,
    pub master_account: UserAccount,
    pub contract: ContractAccount<StakeTokenContractContract>,
    pub contract_owner: UserAccount,
    pub contract_account_id: AccountId,
    pub contract_operator: UserAccount,

    pub users: HashMap<String, UserAccount>,

    pub staking_pool: StakingPoolClient,
    pub staking_service: StakingServiceClient,
    pub account_management: AccountManagementClient,
    pub operator: OperatorClient,
    pub financials: FinancialsClient,
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

    pub fn registered_stake_accounts(&self) -> HashMap<String, StakeAccount> {
        self.users
            .values()
            .fold(HashMap::new(), |mut accounts, user| {
                if let Some(account) = self
                    .account_management
                    .lookup_account(self.master_account(), &user.account_id())
                {
                    accounts.insert(user.account_id().clone(), account);
                }
                accounts
            })
    }

    pub fn process_all_transactions(&self) {
        self.runtime.borrow_mut().process_all().unwrap();
    }
}

pub fn to_valid_account_id(account_id: &str) -> ValidAccountId {
    account_id.try_into().unwrap()
}

pub fn create_context() -> TestContext {
    let (runtime, signer, ..) = init_runtime(None);
    let runtime = Rc::new(RefCell::new(runtime));
    let master_account = UserAccount::new(&runtime, signer); // init_simulator(None);
    let contract_owner = master_account.create_user("oysterpack".to_string(), 1000 * YOCTO);
    let contract_operator = contract_owner.create_user("operator".to_string(), 10 * YOCTO);

    const STAKING_POOL_ID: &str = "astro-stakers-poolv1";
    let staking_pool_id = to_valid_account_id(STAKING_POOL_ID);
    let contract_owner_id = to_valid_account_id(&contract_owner.account_id());
    let contract_operator_id = to_valid_account_id(&contract_operator.account_id());

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
        init_method: new(staking_pool_id, contract_owner_id, contract_operator_id)
    );
    let contract_account_id = contract.user_account.account_id();

    // create 3 user accounts with 1000 NEAR
    let mut users = HashMap::new();
    for i in 1..=3 {
        let account_id = format!("user-{}", i);
        let user_account = master_account.create_user(account_id.clone(), 1000 * YOCTO);
        users.insert(account_id, user_account);
    }

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

    let staking_service = StakingServiceClient::new(&contract_account_id);
    let staking_pool = StakingPoolClient::new(STAKING_POOL_ID, &contract_account_id);
    let account_management = AccountManagementClient::new(&contract_account_id);
    let operator = OperatorClient::new(&contract_account_id);
    let financials = FinancialsClient::new(&contract_account_id);

    TestContext {
        runtime,

        master_account,
        contract,
        contract_account_id,
        contract_owner,
        contract_operator,

        users,
        staking_pool,
        staking_service,
        account_management,
        operator,
        financials,
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
