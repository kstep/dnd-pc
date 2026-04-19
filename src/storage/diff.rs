//! Sparse JSON diff and 3-way merge for cloud-sync reconciliation.
//!
//! See `docs/superpowers/specs/2026-04-18-dirty-field-sync-design.md`.

use std::collections::BTreeSet;

use serde_json::Value;

/// Compute a sparse diff of `current` against `baseline`. Returns `None` if
/// they are structurally equal. Objects are recursed; arrays and primitives
/// are atomic. Keys present in `baseline` but missing from `current` are
/// emitted as `null` (Firestore merge interprets this as field deletion).
pub fn sparse_diff(current: &Value, baseline: &Value) -> Option<Value> {
    if current == baseline {
        return None;
    }
    match (current, baseline) {
        (Value::Object(cur_map), Value::Object(base_map)) => {
            let changed = cur_map
                .iter()
                .filter_map(|(key, cur_val)| match base_map.get(key) {
                    Some(base_val) => sparse_diff(cur_val, base_val).map(|sub| (key.clone(), sub)),
                    None => Some((key.clone(), cur_val.clone())),
                });
            let removed = base_map
                .keys()
                .filter(|key| !cur_map.contains_key(key.as_str()))
                .map(|key| (key.clone(), Value::Null));
            let out: serde_json::Map<String, Value> = changed.chain(removed).collect();
            if out.is_empty() {
                None
            } else {
                Some(Value::Object(out))
            }
        }
        _ => Some(current.clone()),
    }
}

/// 3-way merge: for each field, if `local == baseline` (clean) adopt
/// `remote`; otherwise keep `local` (dirty). Objects recurse; arrays and
/// primitives are atomic.
pub fn merge_3way(local: &Value, baseline: &Value, remote: &Value) -> Value {
    match (local, baseline, remote) {
        (Value::Object(local_map), Value::Object(base_map), Value::Object(remote_map)) => {
            let mut out = serde_json::Map::new();
            // BTreeSet deliberately: N is bounded by Character field count
            // (~17 top-level, ≤20 per nested map), so tree-node overhead is
            // dominated by map size. Sorted iteration gives deterministic
            // key order in the output across calls.
            let mut keys: BTreeSet<&String> = BTreeSet::new();
            keys.extend(local_map.keys());
            keys.extend(base_map.keys());
            keys.extend(remote_map.keys());

            for key in keys {
                let local_val = local_map.get(key);
                let base_val = base_map.get(key);
                let remote_val = remote_map.get(key);

                let clean = local_val == base_val;
                let chosen: Option<Value> = if clean {
                    // Adopt remote (absent if remote doesn't have the key).
                    remote_val.cloned()
                } else {
                    // Dirty locally. Recurse into sub-objects; otherwise keep local atomically.
                    match (local_val, base_val, remote_val) {
                        (
                            Some(lv @ Value::Object(_)),
                            Some(bv @ Value::Object(_)),
                            Some(rv @ Value::Object(_)),
                        ) => Some(merge_3way(lv, bv, rv)),
                        _ => local_val.cloned(),
                    }
                };

                if let Some(v) = chosen {
                    out.insert(key.clone(), v);
                }
            }
            Value::Object(out)
        }
        _ => {
            if local == baseline {
                remote.clone()
            } else {
                local.clone()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::{Skills, SpellSlots};

    #[test]
    fn sparse_diff_identity_returns_none() {
        let a = json!({"str": 10, "dex": 12});
        let b = json!({"str": 10, "dex": 12});
        assert_eq!(sparse_diff(&a, &b), None);
    }

    #[test]
    fn sparse_diff_single_leaf_change() {
        let current = json!({"str": 15, "dex": 12});
        let baseline = json!({"str": 10, "dex": 12});
        assert_eq!(sparse_diff(&current, &baseline), Some(json!({"str": 15})));
    }

    #[test]
    fn sparse_diff_nested_change() {
        let current = json!({"abilities": {"str": 15, "dex": 12}, "hp": 20});
        let baseline = json!({"abilities": {"str": 10, "dex": 12}, "hp": 20});
        assert_eq!(
            sparse_diff(&current, &baseline),
            Some(json!({"abilities": {"str": 15}}))
        );
    }

    #[test]
    fn sparse_diff_array_is_atomic() {
        let current = json!({"weapons": ["sword", "bow"]});
        let baseline = json!({"weapons": ["sword"]});
        assert_eq!(
            sparse_diff(&current, &baseline),
            Some(json!({"weapons": ["sword", "bow"]}))
        );
    }

    #[test]
    fn sparse_diff_key_removed_emits_null() {
        let current = json!({"str": 10});
        let baseline = json!({"str": 10, "player_name": "Bob"});
        assert_eq!(
            sparse_diff(&current, &baseline),
            Some(json!({"player_name": null}))
        );
    }

    #[test]
    fn sparse_diff_key_added_included_whole() {
        let current = json!({"str": 10, "notes": "hi"});
        let baseline = json!({"str": 10});
        assert_eq!(
            sparse_diff(&current, &baseline),
            Some(json!({"notes": "hi"}))
        );
    }

    #[test]
    fn sparse_diff_multiple_branches() {
        let current = json!({"abilities": {"str": 15}, "equipment": {"currency": {"gp": 100}}});
        let baseline = json!({"abilities": {"str": 10}, "equipment": {"currency": {"gp": 50}}});
        assert_eq!(
            sparse_diff(&current, &baseline),
            Some(json!({
                "abilities": {"str": 15},
                "equipment": {"currency": {"gp": 100}}
            }))
        );
    }

    #[test]
    fn sparse_diff_type_change_atomic() {
        let current = json!({"field": 42});
        let baseline = json!({"field": "forty-two"});
        assert_eq!(sparse_diff(&current, &baseline), Some(json!({"field": 42})));
    }

    #[test]
    fn sparse_diff_empty_object_baseline_returns_whole_current() {
        let current = json!({"str": 10, "dex": 12});
        let baseline = json!({});
        assert_eq!(
            sparse_diff(&current, &baseline),
            Some(json!({"str": 10, "dex": 12}))
        );
    }

    #[test]
    fn sparse_diff_null_vs_missing_are_different() {
        // null is explicit tombstone; missing is "not touched". Baseline has
        // key with value, current has key with null → diff shows null.
        let current = json!({"field": null});
        let baseline = json!({"field": "value"});
        assert_eq!(
            sparse_diff(&current, &baseline),
            Some(json!({"field": null}))
        );
    }

    #[test]
    fn merge_3way_all_clean_adopts_remote() {
        let local = json!({"str": 10, "hp": 20});
        let baseline = json!({"str": 10, "hp": 20});
        let remote = json!({"str": 12, "hp": 25});
        assert_eq!(
            merge_3way(&local, &baseline, &remote),
            json!({"str": 12, "hp": 25})
        );
    }

    #[test]
    fn merge_3way_single_dirty_leaf_kept() {
        let local = json!({"str": 15, "hp": 20});
        let baseline = json!({"str": 10, "hp": 20});
        let remote = json!({"str": 10, "hp": 25});
        // str dirty → keep local; hp clean → adopt remote
        assert_eq!(
            merge_3way(&local, &baseline, &remote),
            json!({"str": 15, "hp": 25})
        );
    }

    #[test]
    fn merge_3way_dirty_leaf_wins_over_remote_change() {
        let local = json!({"str": 20});
        let baseline = json!({"str": 10});
        let remote = json!({"str": 30});
        // Both local and remote changed str; local wins (last-writer-wins resolved at
        // next push)
        assert_eq!(merge_3way(&local, &baseline, &remote), json!({"str": 20}));
    }

    #[test]
    fn merge_3way_nested_dirty_path() {
        let local = json!({"abilities": {"str": 15, "dex": 12}});
        let baseline = json!({"abilities": {"str": 10, "dex": 12}});
        let remote = json!({"abilities": {"str": 10, "dex": 14}});
        // str dirty → keep 15; dex clean → adopt 14
        assert_eq!(
            merge_3way(&local, &baseline, &remote),
            json!({"abilities": {"str": 15, "dex": 14}})
        );
    }

    #[test]
    fn merge_3way_array_atomic_clean() {
        let local = json!({"weapons": ["sword"]});
        let baseline = json!({"weapons": ["sword"]});
        let remote = json!({"weapons": ["sword", "bow"]});
        // local == baseline on weapons → adopt remote array whole
        assert_eq!(
            merge_3way(&local, &baseline, &remote),
            json!({"weapons": ["sword", "bow"]})
        );
    }

    #[test]
    fn merge_3way_array_atomic_dirty() {
        let local = json!({"weapons": ["sword", "shield"]});
        let baseline = json!({"weapons": ["sword"]});
        let remote = json!({"weapons": ["sword", "bow"]});
        // local != baseline on weapons → keep local array whole; remote bow is lost
        assert_eq!(
            merge_3way(&local, &baseline, &remote),
            json!({"weapons": ["sword", "shield"]})
        );
    }

    #[test]
    fn merge_3way_remote_drops_clean_key_drops_in_output() {
        let local = json!({"str": 10, "notes": "hi"});
        let baseline = json!({"str": 10, "notes": "hi"});
        let remote = json!({"str": 10});
        // notes clean → adopt remote (absent) → drop from output
        assert_eq!(merge_3way(&local, &baseline, &remote), json!({"str": 10}));
    }

    #[test]
    fn merge_3way_remote_drops_dirty_key_kept() {
        let local = json!({"str": 10, "notes": "edited"});
        let baseline = json!({"str": 10, "notes": "original"});
        let remote = json!({"str": 10});
        // notes dirty → keep local
        assert_eq!(
            merge_3way(&local, &baseline, &remote),
            json!({"str": 10, "notes": "edited"})
        );
    }

    #[test]
    fn merge_3way_local_new_key_kept() {
        // Local added a key not in baseline or remote (e.g. new personality.trait
        // entry).
        let local = json!({"str": 10, "new_field": "added locally"});
        let baseline = json!({"str": 10});
        let remote = json!({"str": 10});
        assert_eq!(
            merge_3way(&local, &baseline, &remote),
            json!({"str": 10, "new_field": "added locally"})
        );
    }

    #[test]
    fn merge_3way_primitive_root_handled() {
        // Degenerate case: not an object at root. Merge rule still applies.
        let local = json!(10);
        let baseline = json!(10);
        let remote = json!(20);
        assert_eq!(merge_3way(&local, &baseline, &remote), json!(20));
    }

    // --- Null-corruption hypothesis: demonstrates how removing a pruned
    // BTreeMap entry (e.g. a skill that drops to ProficiencyLevel::None)
    // produces a `null`-valued field in the Firestore document, which then
    // travels back to localStorage via the subscribe path and breaks
    // Character deserialization on reload.

    #[test]
    fn sparse_diff_removed_nested_skill_emits_null_tombstone() {
        // Baseline: character had proficiency in skill "10" (Nature).
        let baseline = json!({"skills": {"5": 1, "10": 1}});
        // User removes it: Skills::set(Nature, None) prunes the entry.
        let current = json!({"skills": {"5": 1}});
        // Diff emits a null at "10" — Firestore-merge tombstone semantics.
        assert_eq!(
            sparse_diff(&current, &baseline),
            Some(json!({"skills": {"10": null}}))
        );
    }

    #[test]
    fn merge_3way_protracts_null_from_remote_when_local_clean() {
        // Scenario: client A removed skill "10" and pushed the diff to
        // Firestore via merge_doc. Firestore's `setDoc(..., {merge:true})`
        // stores `null` as a literal field value (NOT delete — that would
        // require FieldValue.delete() sentinel which sparse_diff never emits).
        // A subsequent snapshot on client B (or after reload on A with stale
        // baseline) contains the null.
        let local = json!({"skills": {"5": 1, "10": 1}});
        let baseline = json!({"skills": {"5": 1, "10": 1}});
        let remote = json!({"skills": {"5": 1, "10": null}});
        // local == baseline on "skills.10" → adopt remote → null adopted.
        assert_eq!(
            merge_3way(&local, &baseline, &remote),
            json!({"skills": {"5": 1, "10": null}})
        );
    }

    #[test]
    fn merged_skills_with_null_deserializes_as_tombstone_drop() {
        // End-to-end: after merge_3way pipes `null` into `skills`, read-time
        // defence in `Skills::Deserialize` (via
        // `serde_util::deserialize_map_dropping_nulls`) tolerates the
        // tombstone — null entry is dropped, the valid one survives.
        let merged = json!({"skills": {"5": 1, "10": null}});
        let skills: Skills =
            serde_json::from_value(merged["skills"].clone()).expect("skills must load");
        assert_eq!(skills.iter().count(), 1);
    }

    #[test]
    fn merged_spell_slots_with_null_deserializes_as_tombstone_drop() {
        let merged = json!({"spell_slots": {"0": null}});
        let slots: SpellSlots =
            serde_json::from_value(merged["spell_slots"].clone()).expect("spell_slots must load");
        assert!(slots.is_empty());
    }
}
