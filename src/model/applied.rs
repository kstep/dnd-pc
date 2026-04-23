use std::collections::BTreeMap;

use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::vecset::VecSet;

/// Tracks which build decisions have already been realized into `derived`
/// state. Species and background are single-shot booleans; classes track a
/// set of applied levels per class name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Applied {
    #[serde(default)]
    pub species: bool,
    #[serde(default)]
    pub background: bool,
    #[serde(
        default,
        deserialize_with = "crate::serde_util::deserialize_map_dropping_nulls"
    )]
    pub levels: BTreeMap<String, VecSet<u32>>,
}

impl Applied {
    /// True if `level` has already been applied for the given class.
    pub fn contains_level(&self, class: &str, level: u32) -> bool {
        self.levels.get(class).is_some_and(|s| s.contains(&level))
    }

    /// Record `level` as applied for `class`.
    pub fn mark_level(&mut self, class: &str, level: u32) {
        self.levels
            .entry(class.to_string())
            .or_default()
            .insert(level);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn default_is_empty() {
        let applied = Applied::default();
        assert!(!applied.species);
        assert!(!applied.background);
        assert!(applied.levels.is_empty());
    }

    #[test]
    fn deserialize_skips_null_level_tombstones() {
        // Firestore merge-tombstone path — see src/storage/diff.rs
        // `merged_*_with_null_deserializes_as_tombstone_drop` tests for
        // the full end-to-end rationale.
        let value = json!({
            "species": true,
            "background": true,
            "levels": { "Artificer": [1], "Sorcerer": null },
        });
        let applied: Applied = serde_json::from_value(value).expect("must deserialize");
        assert!(applied.contains_level("Artificer", 1));
        assert!(!applied.levels.contains_key("Sorcerer"));
    }

    #[test]
    fn mark_and_query_levels() {
        let mut applied = Applied::default();
        applied.mark_level("Wizard", 1);
        applied.mark_level("Wizard", 2);
        applied.mark_level("Monk", 1);

        assert!(applied.contains_level("Wizard", 1));
        assert!(applied.contains_level("Wizard", 2));
        assert!(!applied.contains_level("Wizard", 3));
        assert!(applied.contains_level("Monk", 1));
    }
}
