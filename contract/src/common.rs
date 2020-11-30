use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    json_types::U128,
};
use std::ops::Deref;

pub type YoctoNEAR = U128;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Hash([u8; 32]);

impl Hash {
    const LENGTH: usize = 32;
}

impl From<&[u8]> for Hash {
    fn from(value: &[u8]) -> Self {
        let mut buf = [0u8; Hash::LENGTH];
        let hash = env::sha256(value);
        buf.copy_from_slice(&hash.as_slice()[..Hash::LENGTH]);
        Self(buf)
    }
}

impl From<&str> for Hash {
    fn from(value: &str) -> Self {
        let mut buf = [0u8; Hash::LENGTH];
        let hash = env::sha256(value.as_bytes());
        buf.copy_from_slice(&hash.as_slice()[..Hash::LENGTH]);
        Self(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::test_utils::near;
    use near_sdk::{testing_env, MockedBlockchain, VMContext};

    #[test]
    fn hash_from_string() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        testing_env!(context);
        let data = "Alfio Zappala";
        let hash = Hash::from(data);
        let hash2 = Hash::from(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn hash_from_bytes() {
        let account_id = near::to_account_id("alfio-zappala.near");
        let mut context = near::new_context(account_id.clone());
        testing_env!(context);
        let data = "Alfio Zappala II";
        let hash = Hash::from(data.as_bytes());
        let hash2 = Hash::from(data.as_bytes());
        assert_eq!(hash, hash2);
    }
}
