use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, ser::SerializeSeq};
use uuid::Uuid;

/// Trait for types that have a `name` field, used by `named_map`.
pub trait Named {
    fn name(&self) -> &str;
}

/// Deserialize a JSON array `[{"name": "Foo", ...}, ...]` into a
/// `BTreeMap<Box<str>, T>` keyed by each element's `name()`.
pub fn named_map<'de, D, T>(deserializer: D) -> Result<BTreeMap<Box<str>, T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Named,
{
    let vec = Vec::<T>::deserialize(deserializer)?;
    Ok(vec
        .into_iter()
        .map(|item| {
            let key: Box<str> = item.name().into();
            (key, item)
        })
        .collect())
}

/// Trait for types keyed by `Uuid`, used by `index_map_as_vec`.
pub trait Keyed {
    fn key(&self) -> Uuid;
}

/// Serialize an `IndexMap<Uuid, T>` as a JSON array and deserialize back,
/// preserving insertion order and O(1) lookup by key.
///
/// Requires `T: Keyed` so the key can be recovered on deserialization.
pub mod index_map_as_vec {
    use super::*;

    pub fn serialize<S, T>(map: &IndexMap<Uuid, T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: serde::Serialize,
    {
        let mut seq = serializer.serialize_seq(Some(map.len()))?;
        for value in map.values() {
            seq.serialize_element(value)?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<IndexMap<Uuid, T>, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: Deserialize<'de> + Keyed,
    {
        let vec = Vec::<T>::deserialize(deserializer)?;
        Ok(vec.into_iter().map(|item| (item.key(), item)).collect())
    }
}
