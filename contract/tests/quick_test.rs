use oysterpack_near_stake_token::near::YOCTO;
use primitive_types::U256;

#[test]
fn quick_test() {
    let stake_near_value_1 = U256::from(YOCTO) * U256::from(4658063269802878999370714u128)
        / U256::from(4579943328412471962879774u128);

    let stake_near_value_2 = U256::from(YOCTO) * U256::from(5658063269802878999370750u128)
        / U256::from(5552942966614588265320892u128);

    let stake_near_value_remainder = (U256::from(YOCTO)
        * U256::from(5658063269802878999370750u128))
        % U256::from(5552942966614588265320892u128);

    println!("{} {}", stake_near_value_1, stake_near_value_1.as_u128());
    println!("{} {} ", stake_near_value_2, stake_near_value_2.as_u128());
    println!("{}", stake_near_value_remainder);
}
