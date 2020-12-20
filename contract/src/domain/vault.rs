use crate::{core::Hash, domain::YoctoStake};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use std::ops::{Deref, DerefMut};

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Vault(pub Hash, pub YoctoStake);

impl Vault {
    /// This information is only needed to validate safe ownership during withdrawal.
    pub fn receiver_id_hash(&self) -> Hash {
        self.0
    }

    /// The remaining amount of tokens in the safe.
    pub fn balance(&self) -> YoctoStake {
        self.1
    }
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Copy, PartialEq, Default)]
pub struct VaultId(pub u128);

impl VaultId {
    pub fn next(&self) -> Self {
        (self.value() + 1).into()
    }
}

impl From<u128> for VaultId {
    fn from(value: u128) -> Self {
        Self(value.into())
    }
}

impl VaultId {
    pub fn value(&self) -> u128 {
        self.0
    }
}

impl Deref for VaultId {
    type Target = u128;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for VaultId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
