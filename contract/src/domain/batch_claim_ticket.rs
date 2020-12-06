use crate::core::Hash;
use crate::domain::BatchId;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::TreeMap;
use near_sdk::serde::Serialize;

/// hash is for account ID
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct BatchClaimTicket(BatchId, Hash);

impl BatchClaimTicket {
    pub fn batch_id(&self) -> BatchId {
        self.0
    }

    pub fn account_hash_id(&self) -> Hash {
        self.1
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct BatchClaimTickets(TreeMap<BatchClaimTicket, [u8; 0]>);

impl BatchClaimTickets {
    pub fn new(key_prefix: [u8; 1]) -> Self {
        Self(TreeMap::new(key_prefix.to_vec()))
    }

    /// returns true if the ticket was inserted, i.e., did not pre-exist
    pub fn insert(&mut self, ticket: &BatchClaimTicket) -> bool {
        self.0.insert(ticket, &[]).is_none()
    }

    pub fn contains(&self, ticket: &BatchClaimTicket) -> bool {
        self.0.contains_key(ticket)
    }

    /// returns true if the ticket existed and was removed
    pub fn remove(&mut self, ticket: &BatchClaimTicket) -> bool {
        self.0.remove(ticket).is_some()
    }

    pub fn iter_from<'a>(
        &'a self,
        ticket: BatchClaimTicket,
    ) -> impl Iterator<Item = BatchClaimTicket> + 'a {
        self.0.iter_from(ticket).map(|(ticket, _)| ticket)
    }

    pub fn iter_rev_from<'a>(
        &'a self,
        ticket: BatchClaimTicket,
    ) -> impl Iterator<Item = BatchClaimTicket> + 'a {
        self.0.iter_rev_from(ticket).map(|(ticket, _)| ticket)
    }

    pub fn iter_rev_from_batch_id<'a>(
        &'a self,
        batch_id: BatchId,
    ) -> impl Iterator<Item = BatchClaimTicket> + 'a {
        self.0
            .iter_rev_from(BatchClaimTicket(batch_id, Hash::default()))
            .map(|(ticket, _)| ticket)
    }

    pub fn len(&self) -> u64 {
        self.0.len()
    }

    pub fn min(&self) -> Option<BatchClaimTicket> {
        self.0.min()
    }

    pub fn max(&self) -> Option<BatchClaimTicket> {
        self.0.max()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::near::*;
    use near_sdk::collections::TreeMap;
    use near_sdk::{serde_json, testing_env, AccountId, MockedBlockchain, VMContext};

    #[test]
    fn batch_claim_tickets() {
        let account_id = "bob.near".to_string();
        let context = new_context(&account_id);
        testing_env!(context);

        let batch_id = BatchId(0);
        let hash = Hash::from(&account_id);
        let ticket = BatchClaimTicket(batch_id, hash);

        let mut bytes = Vec::<u8>::with_capacity(48);
        ticket.serialize(&mut bytes).unwrap();
        println!("bytes.len() = {}", bytes.len());
        println!("{:?}", bytes);

        let mut map = TreeMap::<Vec<u8>, [u8; 0]>::new(vec![2]);
        map.insert(&bytes, &[]);
        assert_eq!(map.len(), 1);
        assert!(map.insert(&bytes, &[]).is_some());
        assert_eq!(map.len(), 1);

        let mut tickets = BatchClaimTickets::new([1]);
        const EMPTY_VALUE: [u8; 0] = [];

        for i in 0..10u128 {
            let ticket = BatchClaimTicket(BatchId(i), Hash::from("bob.near"));
            assert!(tickets.insert(&ticket));
            assert!(!tickets.insert(&ticket));

            let ticket = BatchClaimTicket(BatchId(i), Hash::from("alice.near"));
            assert!(tickets.insert(&ticket));
            assert!(!tickets.insert(&ticket));
        }

        for i in 10..20u128 {
            let ticket = BatchClaimTicket(BatchId(i), Hash::from("roman.near"));
            assert!(tickets.insert(&ticket));
            assert!(!tickets.insert(&ticket));

            let ticket = BatchClaimTicket(BatchId(i), Hash::from("alexander.near"));
            assert!(tickets.insert(&ticket));
            assert!(!tickets.insert(&ticket));
        }

        assert_eq!(tickets.len(), 40);
        assert_eq!(tickets.min().unwrap().0, BatchId(0));
        assert_eq!(tickets.max().unwrap().0, BatchId(19));

        for ticket in tickets
            .iter_from(BatchClaimTicket(BatchId(0), Hash::default()))
            .enumerate()
        {
            println!("{:?}", ticket);
        }

        let tickets_to_delete: Vec<BatchClaimTicket> =
            tickets.iter_rev_from_batch_id(BatchId(10)).collect();
        for ticket in tickets_to_delete.iter() {
            assert!(ticket.batch_id() < BatchId(10));
            assert!(tickets.remove(ticket));
        }
        assert_eq!(tickets.len(), 20);
    }
}
