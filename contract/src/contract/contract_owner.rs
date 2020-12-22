use crate::interface::{ContractOwner, YoctoNear};
//required in order for near_bindgen macro to work outside of lib.rs
use crate::*;
use near_sdk::near_bindgen;

#[near_bindgen]
impl ContractOwner for StakeTokenContract {
    fn owner_id(&self) -> AccountId {
        self.owner_id.clone()
    }

    fn owner_balance(&self) -> YoctoNear {
        (env::account_balance() - self.total_near.amount().value()).into()
    }

    fn stake_all_owner_balance(&mut self) -> YoctoNear {
        self.assert_predecessor_is_owner();
        unimplemented!()
    }

    fn stake_owner_balance(&mut self, amount: YoctoNear) {
        self.assert_predecessor_is_owner();
        unimplemented!()
    }

    fn withdraw_all_owner_balance(&mut self) -> YoctoNear {
        self.assert_predecessor_is_owner();
        unimplemented!()
    }

    fn withdraw_owner_balance(&mut self, amount: YoctoNear) -> YoctoNear {
        self.assert_predecessor_is_owner();
        unimplemented!()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::near::YOCTO;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    fn owner_is_signer() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.signer_account_id = "owner.near".to_string();
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(contract_settings);

        assert_eq!(contract.owner_id, context.signer_account_id);
    }

    #[test]
    fn owner_balance_has_funds() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.signer_account_id = "owner.near".to_string();
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());
        assert_eq!(contract.owner_balance(), (100 * YOCTO).into());

        contract.total_near.credit((50 * YOCTO).into());
        testing_env!(context.clone());
        assert_eq!(contract.owner_balance(), (50 * YOCTO).into());
    }
}
