use hashbrown::HashMap;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use std::ops::{Deref, DerefMut};
use std::{
    convert::TryFrom,
    io::{self, Write},
};

#[derive(Debug, Clone)]
struct HashbrownMap<K, V>(HashMap<K, V>);

impl<K, V> Deref for HashbrownMap<K, V> {
    type Target = HashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K, V> DerefMut for HashbrownMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<K, V> BorshSerialize for HashbrownMap<K, V>
where
    K: BorshSerialize + PartialOrd,
    V: BorshSerialize,
{
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let mut vec = self.0.iter().collect::<Vec<_>>();
        vec.sort_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap());
        u32::try_from(vec.len())
            .map_err(|_| io::ErrorKind::InvalidInput)?
            .serialize(writer)?;
        for (key, value) in vec {
            key.serialize(writer)?;
            value.serialize(writer)?;
        }
        Ok(())
    }
}

impl<K, V> BorshDeserialize for HashbrownMap<K, V>
where
    K: BorshDeserialize + Eq + std::hash::Hash,
    V: BorshDeserialize,
{
    #[inline]
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        let len = u32::deserialize(buf)?;
        // TODO(16): return capacity allocation when we can safely do that.
        let mut result = HashMap::new();
        for _ in 0..len {
            let key = K::deserialize(buf)?;
            let value = V::deserialize(buf)?;
            result.insert(key, value);
        }
        Ok(HashbrownMap(result))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use near_sdk::{AccountId, Balance};

    #[derive(BorshSerialize, BorshDeserialize, Debug)]
    struct Foo {
        x: u64,
        y: String,
        map: HashbrownMap<AccountId, Balance>,
    }

    #[test]
    fn borsh_hashbrown_hashmap() {
        let mut foo = Foo {
            x: 0,
            y: "foo".to_string(),
            map: HashbrownMap(HashMap::new()),
        };
        foo.map.insert("account-id".to_string(), 10);
        let encoded_a = foo.try_to_vec().unwrap();
        let decoded_a: Foo = Foo::try_from_slice(&encoded_a).unwrap();
        assert_eq!(*decoded_a.map.get("account-id").unwrap(), 10);
    }
}
