use blake2::{Blake2b, Digest};
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::LookupMap,
    env,
    json_types::U128,
    AccountId, Balance, BlockHeight, EpochHeight,
};
use std::ops::Deref;

pub type YoctoNEAR = U128;

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Hash([u8; 64]);

impl Hash {
    const LENGTH: usize = 64;
}

impl From<&[u8]> for Hash {
    fn from(value: &[u8]) -> Self {
        let mut buf = [0u8; Hash::LENGTH];
        let hash = Blake2b::digest(value);
        buf.copy_from_slice(&hash.as_slice()[..Hash::LENGTH]);
        Self(buf)
    }
}

impl From<&str> for Hash {
    fn from(value: &str) -> Self {
        let mut buf = [0u8; Hash::LENGTH];
        let hash = Blake2b::digest(value.as_bytes());
        buf.copy_from_slice(&hash.as_slice()[..Hash::LENGTH]);
        Self(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn hash_from_string() {
        let data = "Alfio Zappala";
        let hash = Hash::from(data);
        let hash2 = Hash::from(data);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn hash_from_bytes() {
        let data = "Alfio Zappala II";
        let hash = Hash::from(data.as_bytes());
        let hash2 = Hash::from(data.as_bytes());
        assert_eq!(hash, hash2);
    }
}
