# Dirty-Field Cloud Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix lost-update races on concurrent character edits across multiple browser tabs by switching Firestore pushes from full-document `setDoc` to field-level `setDoc({merge: true})` with a sparse JSON diff, plus a 3-way merge on incoming snapshots.

**Architecture:** Two pure functions on `serde_json::Value` (`sparse_diff`, `merge_3way`) in a new `src/storage/diff.rs`. Runtime path (`queue.rs::execute_op` and `sync.rs::subscribe_to_changes`) and sign-in path (`sync.rs::sync_all_with_cloud`) are rewritten to diff pushes and merge pulls against a RAM-only baseline (`thread_local! HashMap<Uuid, Value>`). Firebase wrapper gains `merge_doc`; `index.html` glue adds `mergeDoc`.

**Tech Stack:** Rust (Leptos 0.8 + `reactive_stores`), `serde_json::Value`, `gloo_storage`, Firebase JS SDK (v11.4.0).

Reference spec: `docs/superpowers/specs/2026-04-18-dirty-field-sync-design.md`

---

## File Structure

**Created:**
- `src/storage/diff.rs` — `sparse_diff`, `merge_3way`, and their unit tests.

**Modified:**
- `src/storage/mod.rs` — declare `mod diff;`.
- `src/firebase.rs` — add `merge_doc(data, path)`.
- `index.html` — add `mergeDoc` to `window.__firebase`.
- `src/storage/sync.rs` — `BASELINES` thread_local, helper fns, rewritten `subscribe_to_changes` character branch and `sync_all_with_cloud` character loop, `delete_character` cleanup, simplified `setup_auto_save` pull effect.
- `src/storage/queue.rs` — rewrite body of `execute_op(CloudOp::PushCharacter)`.

No schema changes. No new localStorage keys. No locale updates.

---

## Task 1: Scaffold `diff.rs` and declare it

**Files:**
- Create: `src/storage/diff.rs`
- Modify: `src/storage/mod.rs`

- [ ] **Step 1: Create the new module file with a placeholder**

Write `src/storage/diff.rs`:

```rust
//! Sparse JSON diff and 3-way merge for cloud-sync reconciliation.
//!
//! See `docs/superpowers/specs/2026-04-18-dirty-field-sync-design.md`.

use serde_json::Value;

/// Compute a sparse diff of `current` against `baseline`. Returns `None` if
/// they are structurally equal. Objects are recursed; arrays and primitives
/// are atomic. Keys present in `baseline` but missing from `current` are
/// emitted as `null` (Firestore merge interprets this as field deletion).
pub fn sparse_diff(_current: &Value, _baseline: &Value) -> Option<Value> {
    todo!()
}

/// 3-way merge: for each field, if `local == baseline` (clean) adopt
/// `remote`; otherwise keep `local` (dirty). Objects recurse; arrays and
/// primitives are atomic.
pub fn merge_3way(_local: &Value, _baseline: &Value, _remote: &Value) -> Value {
    todo!()
}

#[cfg(test)]
mod tests {
    // tests added in later tasks
}
```

- [ ] **Step 2: Declare the module**

In `src/storage/mod.rs`, add `mod diff;` between `mod image;` and `mod local;`. Final module list reads:

```rust
pub mod image;
mod diff;
mod local;
mod migrate;
pub mod queue;
mod sync;
```

- [ ] **Step 3: Build to confirm scaffolding compiles**

Run: `cargo check`
Expected: compiles with warnings about unused `todo!()` only (no errors).

- [ ] **Step 4: Commit**

```bash
git add src/storage/diff.rs src/storage/mod.rs
git commit -m "feat(sync): scaffold diff module"
```

---

## Task 2: Implement and test `sparse_diff`

**Files:**
- Modify: `src/storage/diff.rs`

- [ ] **Step 1: Write failing tests for sparse_diff**

Replace the `#[cfg(test)] mod tests` block in `src/storage/diff.rs`:

```rust
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
        assert_eq!(sparse_diff(&current, &baseline), Some(json!({"notes": "hi"})));
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
        assert_eq!(sparse_diff(&current, &baseline), Some(json!({"field": null})));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib storage::diff::tests::sparse_diff`
Expected: all 10 tests fail with `panic: not yet implemented`.

- [ ] **Step 3: Implement sparse_diff**

Replace the `sparse_diff` stub in `src/storage/diff.rs`:

```rust
pub fn sparse_diff(current: &Value, baseline: &Value) -> Option<Value> {
    if current == baseline {
        return None;
    }
    match (current, baseline) {
        (Value::Object(cur_map), Value::Object(base_map)) => {
            let mut out = serde_json::Map::new();
            for (key, cur_val) in cur_map {
                match base_map.get(key) {
                    Some(base_val) => {
                        if let Some(sub) = sparse_diff(cur_val, base_val) {
                            out.insert(key.clone(), sub);
                        }
                    }
                    None => {
                        out.insert(key.clone(), cur_val.clone());
                    }
                }
            }
            for key in base_map.keys() {
                if !cur_map.contains_key(key) {
                    out.insert(key.clone(), Value::Null);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(Value::Object(out))
            }
        }
        _ => Some(current.clone()),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib storage::diff::tests::sparse_diff`
Expected: all 10 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/storage/diff.rs
git commit -m "feat(sync): sparse_diff for Firestore merge payloads"
```

---

## Task 3: Implement and test `merge_3way`

**Files:**
- Modify: `src/storage/diff.rs`

- [ ] **Step 1: Add failing tests for merge_3way**

Append to the `#[cfg(test)] mod tests` block in `src/storage/diff.rs` (before the closing `}`):

```rust
    #[test]
    fn merge_3way_all_clean_adopts_remote() {
        let local = json!({"str": 10, "hp": 20});
        let baseline = json!({"str": 10, "hp": 20});
        let remote = json!({"str": 12, "hp": 25});
        assert_eq!(merge_3way(&local, &baseline, &remote), json!({"str": 12, "hp": 25}));
    }

    #[test]
    fn merge_3way_single_dirty_leaf_kept() {
        let local = json!({"str": 15, "hp": 20});
        let baseline = json!({"str": 10, "hp": 20});
        let remote = json!({"str": 10, "hp": 25});
        // str dirty → keep local; hp clean → adopt remote
        assert_eq!(merge_3way(&local, &baseline, &remote), json!({"str": 15, "hp": 25}));
    }

    #[test]
    fn merge_3way_dirty_leaf_wins_over_remote_change() {
        let local = json!({"str": 20});
        let baseline = json!({"str": 10});
        let remote = json!({"str": 30});
        // Both local and remote changed str; local wins (last-writer-wins resolved at next push)
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
        assert_eq!(merge_3way(&local, &baseline, &remote), json!({"weapons": ["sword", "bow"]}));
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
        // Local added a key not in baseline or remote (e.g. new personality.trait entry).
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib storage::diff::tests::merge_3way`
Expected: all 10 tests fail with `panic: not yet implemented`.

- [ ] **Step 3: Implement merge_3way**

Replace the `merge_3way` stub in `src/storage/diff.rs`:

```rust
pub fn merge_3way(local: &Value, baseline: &Value, remote: &Value) -> Value {
    match (local, baseline, remote) {
        (Value::Object(local_map), Value::Object(base_map), Value::Object(remote_map)) => {
            let mut out = serde_json::Map::new();
            let mut keys: std::collections::BTreeSet<&String> = std::collections::BTreeSet::new();
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib storage::diff`
Expected: all 20 tests pass (10 sparse_diff + 10 merge_3way).

- [ ] **Step 5: Commit**

```bash
git add src/storage/diff.rs
git commit -m "feat(sync): merge_3way for snapshot reconciliation"
```

---

## Task 4: Firestore `mergeDoc` wrapper

**Files:**
- Modify: `index.html`
- Modify: `src/firebase.rs`

- [ ] **Step 1: Add `mergeDoc` to the JS glue**

In `index.html`, locate the `setDoc:` line in `window.__firebase` (around line 174). Add a new entry directly below it:

```javascript
    setDoc: (data, ...path) => fsSetDoc(fsDoc(db, ...path), data),
    mergeDoc: (data, ...path) => fsSetDoc(fsDoc(db, ...path), data, { merge: true }),
```

- [ ] **Step 2: Add the Rust wrapper**

In `src/firebase.rs`, locate `pub async fn set_doc` (around line 263). Add directly below it:

```rust
pub async fn merge_doc(
    data: &serde_json::Value,
    path: &[&str],
) -> Result<(), FirebaseError> {
    let mut args = vec![to_js(data)?];
    args.extend(path.iter().map(|segment| JsValue::from_str(segment)));
    call_async_with_retry("mergeDoc", &args).await?;
    Ok(())
}
```

Note: `serde_json::Value` is passed directly (not a generic `impl Serialize`) because every caller already has a `Value` in hand — avoids needless generic expansion.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: no errors, no new warnings.

- [ ] **Step 4: Commit**

```bash
git add index.html src/firebase.rs
git commit -m "feat(firebase): add mergeDoc wrapper for field-level writes"
```

---

## Task 5: BASELINES state and helpers

**Files:**
- Modify: `src/storage/sync.rs`

- [ ] **Step 1: Add baseline storage and helpers**

In `src/storage/sync.rs`, locate the existing `thread_local!` block (around line 64):

```rust
thread_local! {
    static SYNC_STATE: RefCell<Option<SyncState>> = const { RefCell::new(None) };
    static SNAPSHOT_SUBSCRIPTION: RefCell<Option<firebase::Subscription>> = const { RefCell::new(None) };
    static AVATAR_SUBSCRIPTION: RefCell<Option<firebase::Subscription>> = const { RefCell::new(None) };
}
```

Replace with (add `BASELINES` and `HashMap` import at the top of the file by replacing `use std::{cell::RefCell, collections::HashSet};` with `use std::{cell::RefCell, collections::{HashMap, HashSet}};`):

```rust
thread_local! {
    static SYNC_STATE: RefCell<Option<SyncState>> = const { RefCell::new(None) };
    static SNAPSHOT_SUBSCRIPTION: RefCell<Option<firebase::Subscription>> = const { RefCell::new(None) };
    static AVATAR_SUBSCRIPTION: RefCell<Option<firebase::Subscription>> = const { RefCell::new(None) };
    static BASELINES: RefCell<HashMap<Uuid, serde_json::Value>> = RefCell::new(HashMap::new());
}
```

- [ ] **Step 2: Add helper functions**

Directly below the `thread_local!` block in `src/storage/sync.rs`, add:

```rust
/// Get a clone of the baseline for `char_id`, or `None` if never synced.
pub(super) fn baseline_get(char_id: &Uuid) -> Option<serde_json::Value> {
    BASELINES.with(|cell| cell.borrow().get(char_id).cloned())
}

/// Replace the baseline for `char_id` with `value`.
pub(super) fn baseline_insert(char_id: Uuid, value: serde_json::Value) {
    BASELINES.with(|cell| {
        cell.borrow_mut().insert(char_id, value);
    });
}

/// Drop the baseline for `char_id`.
pub(super) fn baseline_remove(char_id: &Uuid) {
    BASELINES.with(|cell| {
        cell.borrow_mut().remove(char_id);
    });
}
```

- [ ] **Step 3: Wire baseline_remove into delete_character**

Locate `pub fn delete_character` in `src/storage/sync.rs` (around line 233). Replace:

```rust
pub fn delete_character(id: &Uuid) {
    local::delete_character_local_only(id);

    if let Some(uid) = firebase::current_uid() {
        queue::push(CloudOp::DeleteCharacter { uid, char_id: *id });
    }
}
```

with:

```rust
pub fn delete_character(id: &Uuid) {
    local::delete_character_local_only(id);
    baseline_remove(id);

    if let Some(uid) = firebase::current_uid() {
        queue::push(CloudOp::DeleteCharacter { uid, char_id: *id });
    }
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check`
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add src/storage/sync.rs
git commit -m "feat(sync): BASELINES thread_local + helpers"
```

---

## Task 6: Rewrite queue push to diff against baseline

**Files:**
- Modify: `src/storage/queue.rs`

- [ ] **Step 1: Rewrite `execute_op(CloudOp::PushCharacter)`**

In `src/storage/queue.rs`, locate `CloudOp::PushCharacter` branch inside `execute_op` (around lines 117-125). Replace:

```rust
        CloudOp::PushCharacter { uid, char_id } => {
            let char_key = super::local::character_key(&char_id);
            let Ok(Some(raw)) = LocalStorage::raw().get_item(&char_key) else {
                return Ok(());
            };
            let json: serde_json::Value = serde_json::from_str(&raw)?;
            let char_id_str = char_id.to_string();
            firebase::set_doc(&json, &["users", &uid, "characters", &char_id_str]).await
        }
```

with:

```rust
        CloudOp::PushCharacter { uid, char_id } => {
            let char_key = super::local::character_key(&char_id);
            let Ok(Some(raw)) = LocalStorage::raw().get_item(&char_key) else {
                return Ok(());
            };
            let current: serde_json::Value = serde_json::from_str(&raw)?;
            let baseline = super::sync::baseline_get(&char_id)
                .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
            let Some(diff) = super::diff::sparse_diff(&current, &baseline) else {
                return Ok(());
            };
            let char_id_str = char_id.to_string();
            firebase::merge_doc(&diff, &["users", &uid, "characters", &char_id_str]).await?;
            super::sync::baseline_insert(char_id, current);
            Ok(())
        }
```

Note: `super::diff` resolves because `queue` and `diff` are siblings under `storage`. Requires `diff` to be visible from `queue`; since `diff` is `mod diff;` (private within `storage`), and `queue.rs` is also under `storage`, this works.

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: no errors.

- [ ] **Step 3: Run native test suite**

Run: `cargo test --lib`
Expected: all tests pass (including diff tests from prior tasks; no new failures).

- [ ] **Step 4: Commit**

```bash
git add src/storage/queue.rs
git commit -m "feat(sync): queue push sends sparse diff instead of full doc"
```

---

## Task 7: 3-way merge on snapshot pull

**Files:**
- Modify: `src/storage/sync.rs`

- [ ] **Step 1: Rewrite the character branch of subscribe_to_changes**

In `src/storage/sync.rs`, locate the `subscribe_to_changes` function (around line 414). Replace the whole function body with:

```rust
fn subscribe_to_changes(uid: &str) {
    let last_sync = local::load_last_sync();

    match firebase::subscribe_collection(
        &["users", uid, "characters"],
        &[Where::gt("updated_at", last_sync as f64)],
        move |changes| {
            let mut max_updated = local::load_last_sync();
            let mut dirty = false;

            for change in changes {
                match change.change_type {
                    ChangeType::Added | ChangeType::Modified => {
                        // Migrate remote to current schema, then back to Value for merging.
                        let Some(remote_char) = migrate::deserialize_character_value(change.data)
                        else {
                            log::warn!("Failed to deserialize snapshot character");
                            continue;
                        };
                        let remote_value = match serde_json::to_value(&remote_char) {
                            Ok(v) => v,
                            Err(error) => {
                                log::warn!("Failed to re-serialize remote character: {error}");
                                continue;
                            }
                        };
                        let id = remote_char.id;
                        let remote_ts = remote_char.updated_at;

                        // Load local as Value (via migration-aware load_character).
                        let local_value = local::load_character(&id)
                            .and_then(|c| serde_json::to_value(&c).ok())
                            .unwrap_or_else(|| serde_json::Value::Object(Default::default()));

                        let baseline = baseline_get(&id).unwrap_or_else(|| local_value.clone());
                        let merged = crate::storage::diff::merge_3way(
                            &local_value,
                            &baseline,
                            &remote_value,
                        );

                        let merged_str = match serde_json::to_string(&merged) {
                            Ok(s) => s,
                            Err(error) => {
                                log::warn!("Failed to serialize merged character: {error}");
                                continue;
                            }
                        };
                        if let Err(error) = LocalStorage::raw()
                            .set_item(&local::character_key(&id), &merged_str)
                        {
                            log::warn!("Failed to save merged character: {error}");
                            continue;
                        }
                        baseline_insert(id, remote_value);
                        max_updated = max_updated.max(remote_ts);
                        dirty = true;
                    }
                    ChangeType::Removed => {
                        if let Ok(id) = change.id.parse::<Uuid>() {
                            local::delete_character_local_only(&id);
                            baseline_remove(&id);
                            dirty = true;
                        }
                    }
                }
            }

            if dirty {
                local::save_last_sync(max_updated);
                get_or_init_sync()
                    .index_version
                    .update(|version| *version += 1);
            }
        },
    ) {
        Ok(subscription) => set_snapshot_subscription(subscription),
        Err(error) => log::warn!("Failed to subscribe to character changes: {error}"),
    }
}
```

Note: `crate::storage::diff::merge_3way` path works because `diff` is `mod diff;` inside `storage/mod.rs`; even though private, it's visible from sibling modules within the same crate.

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: no errors. `LocalStorage::raw().set_item` should already be in scope via existing `use gloo_storage::{LocalStorage, Storage};` at the top of `sync.rs`.

- [ ] **Step 3: Commit**

```bash
git add src/storage/sync.rs
git commit -m "feat(sync): 3-way merge on incoming snapshots"
```

---

## Task 8: Initial sync rewrite

**Files:**
- Modify: `src/storage/sync.rs`

- [ ] **Step 1: Rewrite the character loop of sync_all_with_cloud**

In `src/storage/sync.rs`, locate the character loop inside `sync_all_with_cloud` (around lines 556-586). Replace:

```rust
    for remote_value in remote_chars {
        let remote: Character = match migrate::deserialize_character_value(remote_value) {
            Some(character) => character,
            None => {
                log::warn!("Failed to deserialize remote character (migration failed)");
                continue;
            }
        };
        seen_remote.insert(remote.id);

        let local_updated_at = local::load_character(&remote.id)
            .map(|c| c.updated_at)
            .unwrap_or(0);

        if local_updated_at >= remote.updated_at {
            if local_updated_at > remote.updated_at
                && let Some(local_character) = local::load_character(&remote.id)
                && let Err(error) = push_to_cloud(&uid, &local_character).await
            {
                log::warn!("Failed to push local-newer character: {error:?}");
                push_failures += 1;
            }
        } else {
            if let Err(error) = LocalStorage::set(local::character_key(&remote.id), &remote) {
                log::warn!("Failed to save pulled character {}: {error}", remote.id);
                continue;
            }
            max_updated = max_updated.max(remote.updated_at);
            dirty = true;
        }
    }
```

with:

```rust
    for remote_value in remote_chars {
        let remote: Character = match migrate::deserialize_character_value(remote_value) {
            Some(character) => character,
            None => {
                log::warn!("Failed to deserialize remote character (migration failed)");
                continue;
            }
        };
        seen_remote.insert(remote.id);

        let local_character = local::load_character(&remote.id);
        let local_updated_at = local_character.as_ref().map(|c| c.updated_at).unwrap_or(0);
        let char_path = ["users", uid.as_str(), "characters", &remote.id.to_string()];

        if local_updated_at > remote.updated_at {
            // Local has unpushed edits; push them as a diff against remote.
            let Some(local_character) = local_character else {
                continue;
            };
            let local_value = match serde_json::to_value(&local_character) {
                Ok(v) => v,
                Err(error) => {
                    log::warn!("Failed to serialize local character: {error}");
                    continue;
                }
            };
            let remote_value_for_diff = match serde_json::to_value(&remote) {
                Ok(v) => v,
                Err(error) => {
                    log::warn!("Failed to serialize remote character: {error}");
                    continue;
                }
            };
            if let Some(diff) = crate::storage::diff::sparse_diff(&local_value, &remote_value_for_diff) {
                match firebase::merge_doc(&diff, &char_path).await {
                    Ok(()) => baseline_insert(remote.id, local_value),
                    Err(error) => {
                        log::warn!("Failed to push local-newer character: {error:?}");
                        push_failures += 1;
                    }
                }
            } else {
                // No actual diff despite different timestamps → seed baseline from local.
                baseline_insert(remote.id, local_value);
            }
        } else if local_updated_at < remote.updated_at {
            // Remote is newer; blind save and seed baseline from remote.
            if let Err(error) = LocalStorage::set(local::character_key(&remote.id), &remote) {
                log::warn!("Failed to save pulled character {}: {error}", remote.id);
                continue;
            }
            match serde_json::to_value(&remote) {
                Ok(v) => baseline_insert(remote.id, v),
                Err(error) => log::warn!("Failed to seed baseline for {}: {error}", remote.id),
            }
            max_updated = max_updated.max(remote.updated_at);
            dirty = true;
        } else {
            // Timestamps equal → synchronized. Seed baseline.
            match serde_json::to_value(&remote) {
                Ok(v) => baseline_insert(remote.id, v),
                Err(error) => log::warn!("Failed to seed baseline for {}: {error}", remote.id),
            }
        }
    }
```

- [ ] **Step 2: Seed baseline in the push-local-only phase**

In `src/storage/sync.rs`, locate the `if push_local_only { ... }` block inside `sync_all_with_cloud` (around lines 588-605). Replace:

```rust
    if push_local_only {
        for summary in &all_summaries {
            if !seen_remote.contains(&summary.id)
                && let Some(character) = local::load_character(&summary.id)
            {
                log::info!("sync_all_with_cloud: pushing local-only {}", summary.id);
                if let Err(error) = push_to_cloud(&uid, &character).await {
                    log::warn!("Failed to push local-only character: {error:?}");
                    push_failures += 1;
                }
                // Also push stories for local-only characters
                queue::push(CloudOp::PushStories {
                    uid: uid.clone(),
                    char_id: summary.id,
                });
            }
        }
    }
```

with:

```rust
    if push_local_only {
        for summary in &all_summaries {
            if !seen_remote.contains(&summary.id)
                && let Some(character) = local::load_character(&summary.id)
            {
                log::info!("sync_all_with_cloud: pushing local-only {}", summary.id);
                match push_to_cloud(&uid, &character).await {
                    Ok(()) => {
                        if let Ok(v) = serde_json::to_value(&character) {
                            baseline_insert(summary.id, v);
                        }
                    }
                    Err(error) => {
                        log::warn!("Failed to push local-only character: {error:?}");
                        push_failures += 1;
                    }
                }
                // Also push stories for local-only characters
                queue::push(CloudOp::PushStories {
                    uid: uid.clone(),
                    char_id: summary.id,
                });
            }
        }
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: no errors.

- [ ] **Step 4: Run full native test suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/storage/sync.rs
git commit -m "feat(sync): initial sync pushes diffs and seeds baseline"
```

---

## Task 9: Simplify setup_auto_save pull effect

**Files:**
- Modify: `src/storage/sync.rs`

- [ ] **Step 1: Drop the updated_at guard from the pull effect**

In `src/storage/sync.rs`, locate `setup_auto_save` (around line 134). Find the second `Effect::new` inside it (around lines 156-169):

```rust
    let index_version = sync_index_version();
    Effect::new(move |previous: Option<u32>| {
        let (id, local_at) = {
            let character = store.read_untracked();
            (character.id, character.updated_at)
        };
        if previous.is_some()
            && let Some(character) = local::load_character(&id)
            && character.updated_at > local_at
        {
            cloud_updating.update_untracked(|v| *v = true);
            store.set(character);
        }
        index_version.get()
    });
```

Replace with:

```rust
    let index_version = sync_index_version();
    Effect::new(move |previous: Option<u32>| {
        let id = store.read_untracked().id;
        if previous.is_some()
            && let Some(character) = local::load_character(&id)
        {
            cloud_updating.update_untracked(|v| *v = true);
            store.set(character);
        }
        index_version.get()
    });
```

Rationale: after `merge_3way` runs in the snapshot handler, the on-disk character is already the correctly-reconciled state (local dirty fields + remote non-dirty fields). The prior guard `character.updated_at > local_at` would incorrectly block reloads when local was dirty (merged updated_at = local.updated_at = store.updated_at), causing the store to miss adopted remote changes on clean fields. Unconditional reload is simpler and correct.

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: no errors.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --all-targets`
Expected: no new warnings beyond any pre-existing ones.

- [ ] **Step 4: Run nightly fmt**

Run: `cargo +nightly fmt`
Expected: no unstaged reformatting beyond your current changes.

- [ ] **Step 5: Commit**

```bash
git add src/storage/sync.rs
git commit -m "fix(sync): drop updated_at guard in pull effect"
```

---

## Task 10: Full build + manual acceptance

**Files:** none edited — verification only.

- [ ] **Step 1: Run all tests**

Run: `cargo test`
Expected: all tests pass, no regressions.

- [ ] **Step 2: Production-mode build**

Run: `trunk build --release`
Expected: build succeeds. No new warnings.

- [ ] **Step 3: Start dev server**

Run: `trunk serve --port 3000 --open`
Expected: app loads; DevTools console shows Firebase init and the existing "Auth settled" / "sync_all_with_cloud" log lines.

- [ ] **Step 4: Manual acceptance — two browsers**

1. Open the app in two separate browser profiles (or one browser + one incognito), signed in as the same Google account.
2. In profile A, open the same character's Inventory tab.
3. In profile B, open the Stats tab for the same character.
4. Within ~1 s of each other:
   - In A, change the currency (e.g. gp 50 → 100) and commit (blur the field).
   - In B, change STR (e.g. 10 → 20) and commit (blur).
5. Wait ~5 s for the queue debounce and realtime propagation.
6. Verify in **both** profiles:
   - currency = 100
   - STR = 20

Both must show both values. If either value reverted, the bug reproduces — investigate before shipping.

- [ ] **Step 5: Manual acceptance — delete-while-push**

1. Create a fresh character.
2. Edit something to enqueue a push.
3. Before 2 s elapse, delete the character from the list.
4. Verify: character disappears from list AND from Firestore (check across devices or re-login). No zombie push.

- [ ] **Step 6: Final commit (if any cleanup)**

Only if there are uncommitted changes (e.g. auto-formatter touched something):

```bash
git status
# if clean, nothing to do
# otherwise:
# git add <files>
# git commit -m "chore: formatting"
```

---

## Self-Review Summary

- **Spec coverage:** Tasks 2–3 cover `diff.rs`; Task 4 covers the Firestore wrapper and JS glue; Task 5 covers baseline state; Task 6 covers push path; Task 7 covers pull-via-snapshot; Task 8 covers initial sync (both pull-remote-newer, push-local-newer, and push-local-only branches); Task 9 covers the setup_auto_save simplification. Task 1 is scaffolding. Task 10 is acceptance. All spec sections map to a task.
- **Placeholders:** none — every code block is final, every command is the exact invocation.
- **Type consistency:** `sparse_diff` returns `Option<Value>` in both declaration and usage sites (queue push, sync_all_with_cloud). `merge_3way` returns `Value` used identically in the snapshot handler. Baseline API is consistently `baseline_get/insert/remove` with `Uuid` and `serde_json::Value`.
- **No orphan references:** `migrate::deserialize_character_value` (used in Task 7), `local::load_character`, `local::character_key`, `firebase::merge_doc` (added Task 4), and `crate::storage::diff::{sparse_diff, merge_3way}` all defined within or before their first use.
