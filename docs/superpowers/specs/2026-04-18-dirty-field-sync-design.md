# Dirty-Field Cloud Sync Design

Status: approved for planning
Date: 2026-04-18

## Problem

Two browsers, both signed in as the same user, each editing different fields of the same character. Example: browser A edits `equipment.currency` on the inventory tab; browser B edits `abilities.str` on the stats tab. In the current implementation, the browser that hasn't yet received the realtime snapshot from the other one pushes its whole local character document via `firebase::set_doc` (full replace). That push contains stale values for fields the other browser had just changed. The concurrent edit on the other field is silently overwritten on the server.

Root cause: `execute_op(CloudOp::PushCharacter)` in `src/storage/queue.rs` reads the full character from localStorage and pushes it whole; `push_to_cloud` in `src/storage/sync.rs` calls `firebase::set_doc(&character, ...)` — a full-document replace. This is last-writer-wins at the document level. Tab-sleep + 2 s debounce + realtime-propagation delay gives a wide race window.

## Goals

- Concurrent edits to **different leaf fields** never clobber each other.
- Concurrent edits to the **same leaf field** resolve last-writer-wins (unavoidable without CRDTs, accepted as out of scope).
- No schema migration. No new localStorage keys. Forward- and backward-compatible with existing Firestore documents.
- External API of `src/storage/sync.rs` (`setup_auto_save`, `save_and_sync_character`, `delete_character`, etc.) preserved unchanged. Refactor is internal to the storage layer.

## Non-goals

- Concurrent edits to different elements of the same array (e.g. two devices adding different weapons). Arrays are treated as atomic units; one push wins. Solving this requires identity-per-element and `arrayUnion`/`arrayRemove` (variant C from brainstorming), rejected as overkill for single-player usage.
- Offline multi-edit reconciliation across long disconnects (covered well enough by the 3-way merge, edge cases during extended offline periods accepted as rare).

## Approach — diff at push, 3-way merge at pull

### Granularity

Sparse JSON diff to leaf level, **arrays treated atomically**. Firestore's `setDoc({merge: true})` recursively merges nested maps but replaces arrays wholesale, so this granularity matches Firestore's native semantics.

Rejected alternatives:
- Top-level-field granularity: still allows STR vs DEX to collide (both in `abilities`).
- Array-element granularity with `arrayUnion`: requires stable element identity and a distinct push API; too invasive for the payoff.

### Baseline state

**Baseline** = our last-known snapshot of the server's state for a given character. It is the reference we diff against to decide what the user has changed locally since last sync. Baseline is updated in two situations: (1) after a successful push (`baseline = current`), (2) on pull (`baseline = remote`).

Stored in RAM only, as a `thread_local! { static BASELINES: RefCell<HashMap<Uuid, serde_json::Value>> }` in `src/storage/sync.rs`. Not persisted to localStorage.

Rationale: the existing `sync_done` gate in `init_sync` already blocks `touch()` until the first pull completes, which means after page reload the baseline is reconstructed from the initial pull before any edits can generate a diff. Persisting baseline to disk would double localStorage I/O without meaningful benefit for realistic usage.

**Missing baseline fallbacks** (context-dependent):
- On **push** (`execute_op`): fall back to `Value::Object({})`. Diff against empty object yields the whole document, equivalent to a full push. Appropriate for first push of a brand-new or imported character before any sync round-trip.
- On **pull** (`subscribe_to_changes`): fall back to `local`. With `baseline == local`, `merge_3way` degenerates to "remote wins everywhere" (all fields appear clean). Matches the safest default when we have no memory of a prior server state.

### Push path

Pipeline (replaces current `execute_op(CloudOp::PushCharacter)` in `src/storage/queue.rs`):

```
queue flush fires
  ↓
let current_json = localStorage.load(char_id) as serde_json::Value
let baseline = BASELINES.get(char_id).cloned().unwrap_or(Value::Object(default))
let diff = sparse_diff(&current_json, &baseline)
match diff {
    None => return Ok(()),                 // nothing changed — noop
    Some(delta) => {
        firebase::merge_doc(&delta, path).await?
        BASELINES.insert(char_id, current_json)
    }
}
```

Retry semantics unchanged: on error, baseline is not updated, the queue key stays coalesced, next tick re-diffs and re-pushes.

### Pull path

In `subscribe_to_changes` snapshot handler (runtime edits from other devices), replace the current "overwrite localStorage with remote when remote is newer" logic with:

```
for change in changes (Added | Modified):
    let remote = change.data as serde_json::Value
    let local = localStorage.load(remote.id).unwrap_or(default)
    let baseline = BASELINES.get(remote.id).cloned().unwrap_or(local.clone())
    let merged = merge_3way(&local, &baseline, &remote)
    localStorage.save(remote.id, &merged)
    BASELINES.insert(remote.id, remote)
index_version.update(+1)
```

The old `updated_at > local_at` guard is removed. The 3-way merge produces a field-level decision instead of a document-level one.

`setup_auto_save`'s pull effect (triggered on `index_version` changes) simplifies: it reloads the character from localStorage and unconditionally sets the store, since the merge has already resolved the right value per field on disk.

### Initial sync

In `sync_all_with_cloud` at sign-in, for each remote character:

- **No local copy** → save remote as-is; `BASELINES.insert(id, remote)`.
- **Local exists, `local.updated_at < remote.updated_at`** → local is strictly behind (no unpushed edits; `touch()` would have bumped its timestamp). Save remote to localStorage; `BASELINES.insert(id, remote)`.
- **Local exists, `local.updated_at > remote.updated_at`** → local has unpushed edits from a prior session. Compute `diff = sparse_diff(local, remote)`, push via `firebase::merge_doc(diff, ...)`, then `BASELINES.insert(id, local)` (on success; on push failure leave baseline unset — next user edit will retry).
- **Equal timestamps** → synchronized; `BASELINES.insert(id, remote)`.

At this phase there is no Store subscribed and no user input, so the on-disk `local` value equals the "current" state — no 3-way merge needed. The pre-existing logic in `sync_all_with_cloud` already branches on these cases; only the push-branch changes from full `set_doc` to diff-based `merge_doc`, and baseline population is added.

Local-only characters (present locally, absent remotely) are pushed via the existing full-doc `firebase::set_doc` path in the push-local-only phase; baseline is seeded from the pushed character.

On **subsequent** edits (after `setup_auto_save` has wired the Store), push goes through the queue / `execute_op` pipeline, and pull goes through `subscribe_to_changes` — both use `merge_3way` / `sparse_diff` against the live baseline.

## Components

### New module `src/storage/diff.rs`

Two pure functions on `serde_json::Value`:

```rust
pub fn sparse_diff(current: &Value, baseline: &Value) -> Option<Value>;
pub fn merge_3way(local: &Value, baseline: &Value, remote: &Value) -> Value;
```

`sparse_diff` rules:
- If `current == baseline` → `None`.
- If both are JSON `Object`: recurse per key, union of keys; a key present in `baseline` but missing from `current` emits `null` in the diff (Firestore merge interprets `null` as field deletion); a key present in `current` but missing from `baseline` is included whole.
- Any other case (both arrays, both primitives, types differ): atomic — include `current` whole in the diff if it differs from `baseline`.

`merge_3way` rules: for each field in the union of keys in `local`, `baseline`, `remote`:
- If `local == baseline` (not dirty), output = `remote[field]` (or absent if `remote` doesn't have it).
- If `local != baseline` (dirty), output = `local[field]`. Baseline update is caller's responsibility.
- Recurse for nested objects; arrays/primitives compared atomically.

Both functions are pure, no I/O. Unit-testable in native `cargo test`.

### Modified `src/storage/sync.rs`

- Add `thread_local! BASELINES: RefCell<HashMap<Uuid, serde_json::Value>>`.
- `subscribe_to_changes` character branch: replace blind overwrite with `merge_3way(local, baseline_or_local, remote)`, persist merged to localStorage, `BASELINES.insert(id, remote)`.
- `sync_all_with_cloud` character loop: push branch switches from `firebase::set_doc(&full_local)` to `firebase::merge_doc(&sparse_diff(local, remote))`; pull branch unchanged (blind save); both branches seed baseline (local on push, remote on pull).
- Add internal helpers: `baseline_get`, `baseline_insert`, `baseline_remove`.
- Wire `baseline_remove` into `delete_character` alongside `local::delete_character_local_only`.

No changes to `setup_auto_save` body beyond the pull-effect simplification noted above.

### Modified `src/storage/queue.rs`

- `execute_op(CloudOp::PushCharacter)` rewritten per the push-path pipeline above.
- Queue structure, `QueueKey` coalescing, flush interval unchanged.

### Modified `src/firebase.rs`

Add:

```rust
pub async fn merge_doc(data: &Value, path: &[&str]) -> Result<(), FirebaseError>;
```

Signature parallel to existing `set_doc`, differing only in passing `{ merge: true }` to the underlying Firestore call.

### Modified `index.html`

Extend `window.__firebase` glue:

```javascript
mergeDoc: (data, ...path) => fsSetDoc(fsDoc(db, ...path), data, { merge: true }),
```

No other index.html changes.

## Data flow examples

### Example 1 — disjoint fields (the reported bug)

Start: both browsers synced, `{abilities:{str:10}, equipment:{currency:{gp:50}}}`.

Browser A edits currency → `current_A.equipment.currency.gp = 100`. On debounce, `sparse_diff(current_A, baseline_A) = {equipment:{currency:{gp:100}}}`. Pushes via `merge_doc`. After ack, `baseline_A = current_A`.

Browser B edits str → `current_B.abilities.str = 20`. Snapshot from A arrives at B before B pushes: `remote = {abilities:{str:10}, equipment:{currency:{gp:100}}, updated_at:...}`. `merge_3way(local_B, baseline_B, remote)` yields `{abilities:{str:20}, equipment:{currency:{gp:100}}}` (str kept because dirty, currency adopted because clean). `baseline_B = remote`.

B's debounce fires: `sparse_diff({str:20, ...currency:100}, baseline={str:10, ...currency:100}) = {abilities:{str:20}}`. Pushes just STR. Server merges → final state `{str:20, currency:100}`. Both converge. No loss.

### Example 2 — same field concurrently

Both edit STR. Last pusher's value reaches the server last and wins at the merge. The earlier pusher receives the later value via snapshot; since its `current == baseline` (already pushed and baseline updated), it adopts the newer remote value. Convergence to last-writer. Accepted.

### Example 3 — arrays

Both edit `equipment.weapons`. Whichever pushes second replaces the array. One entry is lost. Known limitation (variant B in brainstorming).

### Example 4 — optional field cleared

User clears `identity.player_name` (an `Option<String>`). Serde emits `null`. Diff: `{identity:{player_name:null}}`. Firestore `setDoc({merge:true}, {...player_name:null})` deletes the field. Correct behavior.

## Error handling

- **Push network/auth failure**: baseline stays at prior value; next queue tick re-diffs identical payload and retries. Idempotent.
- **Serialization to `Value` failure**: treated as fatal for that push; logged; skipped. Unreachable for our model types in practice (all `Serialize`-safe).
- **Snapshot deserialization failure**: already handled in existing code (`log::warn` + skip).
- **Missing baseline**: covered by context-dependent fallbacks described in "Baseline state" above.
- **Delete coalescing**: existing `QueueKey::Character` ensures `CloudOp::DeleteCharacter` inserted after `PushCharacter` replaces the push entry. `delete_character` also calls `baseline_remove`.

## Testing

### Unit tests — `src/storage/diff.rs`

Native `cargo test` (pure functions, no WASM dependencies).

`sparse_diff`:
- identity → `None`
- single leaf change
- nested multi-level change
- array changed → whole array in diff
- object key present in baseline, missing in current → `null` in diff
- object key present in current, missing in baseline → included in diff
- multiple disjoint branches changed
- type change at a key (e.g. string → number)

`merge_3way`:
- all fields clean → output equals remote
- one dirty leaf → current kept at that leaf, rest from remote
- dirty leaf + remote also changed same leaf → current kept (last-writer-wins via next push)
- nested dirty path
- array clean → remote array adopted; array dirty → local array kept
- remote drops a key clean locally → key absent in output
- remote drops a key dirty locally → key kept in output

### Acceptance — manual

1. Two browsers signed in as the same user.
2. Tab A on inventory, tab B on stats.
3. Edit currency in A, edit STR in B within the debounce window.
4. Wait for realtime propagation.
5. Verify: both browsers show new currency AND new STR.

No integration test with Firestore mocks — the contract is (a) diff is correct (covered by unit tests) and (b) Firestore's documented merge semantics hold (trusted).

## Rollout

No schema migration. No new localStorage keys. Existing full-document pushes in Firestore remain readable — merge pushes and full-document sets are interchangeable write operations producing the same document shape. Clients running old code and clients running new code can coexist indefinitely; old clients still full-replace (and may still cause the lost-update bug for their own edits against new-client writes), new clients always diff-push.

Deploy as a single PR.

## Open questions / future work

- If array-atomic collisions become a real problem (e.g. shared-campaign inventory co-editing), revisit variant C (identity-per-element + `arrayUnion`/`arrayRemove`).
- Potential observability: log diff key count on each push to spot surprising full-document diffs. Optional, not in scope.
