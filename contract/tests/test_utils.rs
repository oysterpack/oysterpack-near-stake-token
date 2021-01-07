extern crate oysterpack_near_stake_token;

use near_sdk_sim::*;

use oysterpack_near_stake_token::{near::YOCTO, ContractSettings, StakeTokenContractContract};

lazy_static! {
    static ref WASM_BYTES: &'static [u8] =
        include_bytes!("../res/oysterpack_near_stake_token.wasm").as_ref();
}

pub struct TestContext {
    pub master_account: UserAccount,
    pub contract: ContractAccount<StakeTokenContractContract>,
    pub contract_owner: UserAccount,
    pub contract_operator: UserAccount,
    pub settings: ContractSettings,
}

pub fn create_context() -> TestContext {
    let master_account = init_simulator(None);
    let contract_owner = master_account.create_user("oysterpack".to_string(), 1000 * YOCTO);
    let contract_operator = contract_owner.create_user("operator".to_string(), 10 * YOCTO);

    let settings = ContractSettings::new(
        "astro-stakers.poolv1.near".to_string(),
        contract_operator.account_id.clone(),
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

    TestContext {
        master_account,
        contract,
        contract_owner,
        contract_operator,
        settings,
    }
}