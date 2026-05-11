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
    pub levels: BTreeMap<Box<str>, VecSet<u32>>,
}

impl Applied {
    /// True if `level` has already been applied for the given class.
    pub fn contains_level(&self, class: &str, level: u32) -> bool {
        self.levels
            .get(class)
            .is_some_and(|levels| levels.contains(&level))
    }

    /// Record `level` as applied for `class`.
    pub fn mark_level(&mut self, class: &str, level: u32) {
        self.levels
            .entry(Box::from(class))
            .or_default()
            .insert(level);
    }

    /// Wipe all applied flags so pending-discovery starts from a clean
    /// slate (otherwise a flag set on a speculative cascade snapshot
    /// would mask features that legitimately need re-surfacing).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Wipe per-class level flags only, preserving `species`/`background`
    /// booleans. The targeted form for fresh discovery from a speculative
    /// snapshot — cascade-on-clone stamps `applied.classes[X][N]=true` for
    /// the picked level, which would otherwise mask the L_N features that
    /// still need to surface for modal input. Resetting *all* applied flags
    /// would also wipe species so pre-applied origin features (e.g.
    /// Versatile already replaced by an Origin pick) would re-emerge as
    /// pending.
    pub fn reset_class_levels(&mut self) {
        self.levels.clear();
    }

    /// Drop applied flags for the given (class, levels) range, leaving
    /// every other level alone. Lets the caller surface a freshly cascaded
    /// slice of levels without re-emitting placeholders (Subclass/ASI)
    /// that were already resolved at lower levels.
    pub fn forget_class_levels(&mut self, class: &str, levels: impl IntoIterator<Item = u32>) {
        if let Some(set) = self.levels.get_mut(class) {
            for level in levels {
                set.remove(&level);
            }
        }
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
