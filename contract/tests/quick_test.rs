#![allow(unused_imports)]

use oysterpack_near_stake_token::domain::{BlockTimeHeight, StakeTokenValue};
use oysterpack_near_stake_token::near::YOCTO;
use primitive_types::U256;

#[test]
fn quick_test() {
    let balance_history: Vec<u128> = vec![
        72145722678713040200000000,
        72145780435590802700000000, // register account
        72145841274282485800000000, // register account
    ];

    let mut i = 0;
    while i < balance_history.len() - 1 {
        let balance_1 = &balance_history[i];
        let balance_2 = &balance_history[i + 1];

        if balance_2 > balance_1 {
            println!(
                "{} | {} | {}",
                balance_2 - balance_1,
                (balance_2 - balance_1) as f64 / YOCTO as f64,
                YOCTO / (balance_2 - balance_1)
            );
        } else {
            println!(
                "balance went down by: {} | {}",
                balance_1 - balance_2,
                (balance_1 - balance_2) as f64 / YOCTO as f64
            );
        }

        i += 1;
    }
}
