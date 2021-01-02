use near_sdk::{json_types::U128, serde_json};
use std::collections::HashMap;

#[test]
fn quick_test() {
    let pending_withdrawal = 11946810934771951073054235u128;
    let unstaked_near = 1010187198366539620603257;

    let alfio_zappala_redeem_batch_value = 12956998133138490693657492u128;

    println!("{}", pending_withdrawal + unstaked_near);
    println!(
        "{}",
        alfio_zappala_redeem_batch_value - pending_withdrawal - unstaked_near
    );
}
