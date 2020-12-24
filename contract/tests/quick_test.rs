use near_sdk::{json_types::U128, serde_json};

#[test]
fn quick_test() {
    let optional_num: Option<U128> = Some(100.into());

    println!("{}", serde_json::to_string(&optional_num).unwrap());

    let optional_num: Option<U128> = serde_json::from_str(r#"null"#).unwrap();

    let value: Option<String> = Some("".to_string());
    match value {
        None => println!("value is None"),
        Some(value) if value.is_empty() => println!("value is empty string"),
        Some(value) => println!("value is {}", value),
    }
}
