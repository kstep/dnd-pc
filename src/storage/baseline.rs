//! Per-character server baseline — last-known Firestore state used to
//! compute sparse diffs on push and resolve 3-way merges on pull.
//!
//! See `docs/superpowers/specs/2026-04-18-dirty-field-sync-design.md`.

use std::{cell::RefCell, collections::HashMap};

use serde_json::Value;
use uuid::Uuid;

thread_local! {
    static STORE: RefCell<HashMap<Uuid, Value>> = RefCell::new(HashMap::new());
}

/// Run `f` with a borrow of the baseline for `char_id` (or `None` if never
/// synced). Borrowing the stored `Value` avoids cloning a ~50 KB character
/// tree on every push/pull.
pub fn with<R>(char_id: &Uuid, f: impl FnOnce(Option<&Value>) -> R) -> R {
    STORE.with(|cell| f(cell.borrow().get(char_id)))
}

/// Replace the baseline for `char_id` with `value`.
pub fn insert(char_id: Uuid, value: Value) {
    STORE.with(|cell| {
        cell.borrow_mut().insert(char_id, value);
    });
}

/// Drop the baseline for `char_id`.
pub fn remove(char_id: &Uuid) {
    STORE.with(|cell| {
        cell.borrow_mut().remove(char_id);
    });
}
