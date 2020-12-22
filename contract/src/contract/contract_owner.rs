use crate::interface::{ContractOwner, YoctoNear};
//required in order for near_bindgen macro to work outside of lib.rs
use crate::*;
use near_sdk::json_types::ValidAccountId;
use near_sdk::near_bindgen;

#[near_bindgen]
impl ContractOwner for StakeTokenContract {
    fn owner_id(&self) -> AccountId {
        self.owner_id.clone()
    }

    fn owner_balance(&self) -> YoctoNear {
        let contract_account_balance = env::account_balance();
        let total_customer_accounts_unstaked_balance = self.total_near.amount().value();
        let customer_batched_stake_deposits = self
            .stake_batch
            .map_or(0, |batch| batch.balance().amount().value())
            + self
                .next_stake_batch
                .map_or(0, |batch| batch.balance().amount().value());
        (contract_account_balance
            - total_customer_accounts_unstaked_balance
            - customer_batched_stake_deposits)
            .into()
    }

    fn transfer_ownership(&mut self, new_owner: ValidAccountId) {
        self.owner_id = new_owner.into()
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

    #[test]
    fn owner_balance_has_funds_with_pending_stake_batches() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        context.signer_account_id = "owner.near".to_string();
        context.is_view = false;
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        *contract.batch_id_sequence += 1;
        contract.stake_batch = Some(domain::StakeBatch::new(
            contract.batch_id_sequence,
            YOCTO.into(),
        ));
        *contract.batch_id_sequence += 1;
        contract.next_stake_batch = Some(domain::StakeBatch::new(
            contract.batch_id_sequence,
            (2 * YOCTO).into(),
        ));

        context.account_balance = 100 * YOCTO;
        testing_env!(context.clone());
        assert_eq!(contract.owner_balance(), (97 * YOCTO).into());

        contract.total_near.credit((50 * YOCTO).into());
        testing_env!(context.clone());
        assert_eq!(contract.owner_balance(), (47 * YOCTO).into());
    }
}
