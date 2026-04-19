use std::{collections::BTreeMap, fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};

/// Deserialize a `BTreeMap<K, V>` tolerating `null` values: entries whose
/// value is `null` are dropped and a warning is logged.
///
/// Use via `#[serde(deserialize_with = "deserialize_map_dropping_nulls")]`
/// on fields typed as `BTreeMap<K, V>`.
///
/// The path that introduces such `null` values is `sparse_diff`
/// (`src/storage/diff.rs`), which emits a literal `null` for removed map
/// entries as a Firestore-merge tombstone. Firestore's `setDoc({merge:true})`
/// stores that `null` as a value (deletion requires the `FieldValue.delete()`
/// sentinel), and it can come back through `merge_3way` into localStorage.
/// Tolerating the null on read avoids cascading an "Invalid character file"
/// for an otherwise-recoverable tombstone.
pub fn deserialize_map_dropping_nulls<'de, D, K, V>(
    deserializer: D,
) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord + fmt::Debug,
    V: Deserialize<'de>,
{
    deserializer.deserialize_map(NullTolerantMapVisitor(PhantomData))
}

struct NullTolerantMapVisitor<K, V>(PhantomData<fn() -> (K, V)>);

impl<'de, K, V> Visitor<'de> for NullTolerantMapVisitor<K, V>
where
    K: Deserialize<'de> + Ord + fmt::Debug,
    V: Deserialize<'de>,
{
    type Value = BTreeMap<K, V>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a map tolerating null values (dropped on read)")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<Self::Value, M::Error> {
        let mut out = BTreeMap::new();
        while let Some(key) = access.next_key::<K>()? {
            match access.next_value::<Option<V>>()? {
                None => log::warn!("dropping null map entry for key {key:?}"),
                Some(value) => {
                    out.insert(key, value);
                }
            }
        }
        Ok(out)
    }
}
