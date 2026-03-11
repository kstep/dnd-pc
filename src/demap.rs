use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, de, ser::SerializeSeq};
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

/// Deserialize a `BTreeMap<u32, V>` accepting both numeric keys (binary
/// formats) and stringified numbers (JSON).
pub fn u32_key_map<'de, D, V>(deserializer: D) -> Result<BTreeMap<u32, V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    struct Visitor<V>(std::marker::PhantomData<V>);

    impl<'de, V: Deserialize<'de>> de::Visitor<'de> for Visitor<V> {
        type Value = BTreeMap<u32, V>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a map with u32 keys (numeric or stringified)")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: de::MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<FlexU32, V>()? {
                result.insert(key.0, value);
            }
            Ok(result)
        }
    }

    deserializer.deserialize_map(Visitor(std::marker::PhantomData))
}

struct FlexU32(u32);

impl<'de> Deserialize<'de> for FlexU32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FlexU32Visitor;

        impl<'de> de::Visitor<'de> for FlexU32Visitor {
            type Value = FlexU32;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("u32 or stringified u32")
            }

            fn visit_u32<E: de::Error>(self, v: u32) -> Result<FlexU32, E> {
                Ok(FlexU32(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<FlexU32, E> {
                u32::try_from(v).map(FlexU32).map_err(de::Error::custom)
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<FlexU32, E> {
                v.parse().map(FlexU32).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_any(FlexU32Visitor)
    }
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
