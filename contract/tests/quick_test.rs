use oysterpack_near_stake_token::domain::{BlockTimeHeight, StakeTokenValue};
use oysterpack_near_stake_token::near::YOCTO;
use primitive_types::U256;

#[test]
fn quick_test() {
    let stake_token_value = StakeTokenValue::new(
        BlockTimeHeight::default(),
        7000000000000000000000001_u128.into(),
        1749999999999999999999998_u128.into(),
    );

    println!(
        "1 NEAR = {} STAKE",
        stake_token_value.near_to_stake(YOCTO.into())
    );
    println!(
        "499999999999999999999999 STAKE = {} NEAR",
        stake_token_value.stake_to_near(499999999999999999999999.into())
    );

    let stake_token_value = StakeTokenValue::new(
        BlockTimeHeight::default(),
        9000000000000000000000002_u128.into(),
        2249999999999999999999996_u128.into(),
    );

    println!(
        "1 NEAR = {} STAKE",
        stake_token_value.near_to_stake(1.into())
    );
    println!(
        "1 STAKE = {} NEAR",
        stake_token_value.stake_to_near(1.into())
    );

    let total_staked_near_balance = U256::from(2249999999999999999999996_u128)
        * U256::from(stake_token_value.stake_to_near(YOCTO.into()))
        / U256::from(YOCTO);
    println!(
        "total_staked_near_balance =  {} NEAR",
        total_staked_near_balance
    );
}
