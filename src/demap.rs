use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, de};

/// Trait for types that have a `name` field, used by `named_map`.
pub trait Named {
    fn name(&self) -> &str;
}

/// Deserialize a JSON array `[{"name": "Foo", ...}, ...]` into a
/// `BTreeMap<String, T>` keyed by each element's `name()`.
pub fn named_map<'de, D, T>(deserializer: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Named,
{
    let vec = Vec::<T>::deserialize(deserializer)?;
    Ok(vec
        .into_iter()
        .map(|item| {
            let key = item.name().to_string();
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
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> de::Visitor<'de> for V {
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

        d.deserialize_any(V)
    }
}
