use crate::{core::Hash, domain::YoctoStake, interface};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U128;
use std::ops::{Deref, DerefMut};

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Vault(pub Hash, pub YoctoStake);

impl Vault {
    /// This information is only needed to validate safe ownership during withdrawal.
    pub fn owner_id_hash(&self) -> Hash {
        self.0
    }

    /// The remaining amount of tokens in the safe.
    pub fn balance(&self) -> YoctoStake {
        self.1
    }

    pub fn debit(&mut self, amount: YoctoStake) {
        *self.1 -= *amount
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

impl From<U128> for VaultId {
    fn from(value: U128) -> Self {
        Self(value.0)
    }
}

impl From<interface::VaultId> for VaultId {
    fn from(id: interface::VaultId) -> Self {
        id.0 .0.into()
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

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn debit() {
        let mut vault = Vault(Hash::default(), YoctoStake(100));
        vault.debit(YoctoStake(10));
        assert_eq!(vault.balance(), YoctoStake(90));
    }

    #[test]
    fn inc_vault_sequence_id() {
        let mut vault_id = VaultId::default();
        *vault_id += 1;
        assert_eq!(vault_id.value(), 1);
    }
}
