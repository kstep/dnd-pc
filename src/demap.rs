use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};

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
