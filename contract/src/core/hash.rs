
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
};
use std::convert::TryInto;

#[derive(
    BorshDeserialize,
    BorshSerialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Debug,
    Ord,
    PartialOrd,
    Default,
)]
pub struct Hash([u8; 32]);

impl Hash {
    const LENGTH: usize = 32;
}

impl From<[u8; 32]> for Hash {
    fn from(value: [u8; 32]) -> Self {
        Self(value)
    }
}

impl From<&[u8]> for Hash {
    fn from(value: &[u8]) -> Self {
        assert!(value.len() > 0, "value cannot be empty");
        let hash = env::sha256(value);
        Self(hash.try_into().unwrap())
    }
}

impl From<&str> for Hash {
    fn from(value: &str) -> Self {
        assert!(value.len() > 0, "value cannot be empty");
        let hash = env::sha256(value.as_bytes());
        Self(hash.try_into().unwrap())
    }
}

impl From<&String> for Hash {
    fn from(value: &String) -> Self {
        value.as_str().into()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;
    use near_sdk::{testing_env, MockedBlockchain};

    #[test]
    fn hash_from_string() {
        let account_id = "alfio-zappala.near".to_string();
        let context = new_context(&account_id);
        testing_env!(context);
        let data = "Alfio Zappala";
        let hash = Hash::from(data);
        let hash2 = Hash::from(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    #[should_panic(expected = "value cannot be empty")]
    fn hash_from_empty_string() {
        let account_id = "alfio-zappala.near".to_string();
        let context = new_context(&account_id);
        testing_env!(context);
        Hash::from("");
    }

    #[test]
    fn hash_from_bytes() {
        let account_id = "alfio-zappala.near".to_string();
        let context = new_context(&account_id);
        testing_env!(context);
        let data = "Alfio Zappala II";
        let hash = Hash::from(data.as_bytes());
        let hash2 = Hash::from(data.as_bytes());
        assert_eq!(hash, hash2);
    }

    #[test]
    #[should_panic(expected = "value cannot be empty")]
    fn hash_from_empty_bytes() {
        let account_id = "alfio-zappala.near".to_string();
        let context = new_context(&account_id);
        testing_env!(context);
        Hash::from("".as_bytes());
    }
}
