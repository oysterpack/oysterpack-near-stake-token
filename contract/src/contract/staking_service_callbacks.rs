//required in order for near_bindgen macro to work outside of lib.rs
#[allow(unused_imports)]
use crate::*;
use crate::{
    domain, interface::StakeTokenValue, near::assert_predecessor_is_self,
    staking_pool_failures::GET_STAKED_BALANCE_FAILURE, StakeTokenContract,
};
use near_sdk::{json_types::U128, near_bindgen};

type Balance = U128;

#[near_bindgen]
impl StakeTokenContract {
    pub fn on_get_account_staked_balance(
        &self,
        #[callback] staked_balance: Balance,
    ) -> StakeTokenValue {
        assert_predecessor_is_self();
        assert!(self.promise_result_succeeded(), GET_STAKED_BALANCE_FAILURE);
        domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.amount()).into()
    }

    /// updates the cached [StakeTokenValue]
    pub fn on_refresh_account_staked_balance(
        &mut self,
        #[callback] staked_balance: Balance,
    ) -> StakeTokenValue {
        assert_predecessor_is_self();
        assert!(self.promise_result_succeeded(), GET_STAKED_BALANCE_FAILURE);
        self.stake_token_value =
            domain::StakeTokenValue::new(staked_balance.0.into(), self.total_stake.amount());
        self.stake_token_value.into()
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::domain::TimestampedStakeBalance;
    use crate::{near::YOCTO, test_utils::*};
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    fn on_get_account_staked_balance_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        contract.total_stake.credit(YOCTO.into());
        let stake_token_value = contract.on_get_account_staked_balance(YOCTO.into());
        assert_eq!(
            stake_token_value.total_stake_supply,
            contract.total_stake.amount().into()
        );
        assert_eq!(stake_token_value.total_staked_near_balance, YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "func call is only allowed internally")]
    fn on_get_account_staked_balance_success_should_only_be_invoked_by_self() {
        let account_id = "alfio-zappala.near";
        let context = new_context(account_id);
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let contract = StakeTokenContract::new(contract_settings);
        contract.on_get_account_staked_balance(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "failed to get staked balance from staking pool")]
    fn on_get_account_staked_balance_failure() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        set_env_with_failed_promise_result(&mut contract);
        contract.total_stake.credit(YOCTO.into());
        contract.on_get_account_staked_balance(YOCTO.into());
    }

    #[test]
    fn on_refresh_account_staked_balance_success() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        contract.total_stake = TimestampedStakeBalance::new((100 * YOCTO).into());

        context.epoch_height += 1;
        context.block_timestamp += 1000;
        context.block_index += 100;
        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());
        contract.on_refresh_account_staked_balance((110 * YOCTO).into());

        let stake_token_value = contract.stake_token_value;
        assert_eq!(
            stake_token_value.total_stake_supply(),
            contract.total_stake.amount()
        );
        assert_eq!(
            stake_token_value.total_staked_near_balance().value(),
            110 * YOCTO
        );
        assert_eq!(
            stake_token_value
                .block_time_height()
                .block_timestamp()
                .value(),
            context.block_timestamp
        );
        assert_eq!(
            stake_token_value.block_time_height().block_height().value(),
            context.block_index
        );
        assert_eq!(
            stake_token_value.block_time_height().epoch_height().value(),
            context.epoch_height
        );
    }

    #[test]
    #[should_panic(expected = "failed to get staked balance from staking pool")]
    fn on_refresh_account_staked_balance_failure() {
        let account_id = "alfio-zappala.near";
        let mut context = new_context(account_id);
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);

        // callback can only be invoked from itself
        context.predecessor_account_id = context.current_account_id.clone();
        testing_env!(context.clone());

        set_env_with_failed_promise_result(&mut contract);
        contract.on_refresh_account_staked_balance(YOCTO.into());
    }

    #[test]
    #[should_panic(expected = "func call is only allowed internally")]
    fn on_refresh_account_staked_balance_should_only_be_invoked_by_self() {
        let account_id = "alfio-zappala.near";
        let context = new_context(account_id);
        testing_env!(context.clone());

        let contract_settings = default_contract_settings();
        let mut contract = StakeTokenContract::new(contract_settings);
        contract.on_refresh_account_staked_balance(YOCTO.into());
    }
}
