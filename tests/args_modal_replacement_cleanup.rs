//! Regression contract for the args modal's `ReplacementPicker` cleanup.
//!
//! Bug: on uncheck, the picker drops the inner `<Show>` body that owns
//! the replacement's ARG `RwSignal`/`StoredValue`. If the modal-scoped
//! `all_signals` / `all_dice` maps still hold references to those
//! `StoredValue`s, the next form submit walks the map with non-`try_`
//! `with_value` / `get_untracked` and panics:
//!
//! > "you tried to access a reactive value ... but it has already been
//! >  disposed."
//!
//! `event.prevent_default()` already ran, so the modal handler aborts
//! silently — first click does nothing visible. Subsequent clicks land
//! in a poisoned reactive runtime; the form ends up submitting through
//! the browser, reloading the page.
//!
//! The fix (in `args_modal.rs::ReplacementPicker`) extracts a shared
//! cleanup helper and calls it from BOTH `on_input(_, None)` and the
//! checkbox `on:change` uncheck branch — the latter previously
//! skipped cleanup. These tests assert that contract directly using
//! the same data shape as the production code.

use std::collections::BTreeMap;

use leptos::prelude::*;

type SignalsMap = RwSignal<BTreeMap<String, StoredValue<Vec<RwSignal<i32>>>>>;

/// Mirror of the modal: a long-lived parent scope owns the map, a
/// child scope (the `<Show when=replacing>` body) registers the
/// replacement's signals under its `(name, source)` key and gets
/// dropped on uncheck. Returns parent + child + map; the parent must
/// stay alive for the duration of the test (otherwise `all_signals`
/// itself is disposed and submit_walk panics with the bug message
/// before the test ever reaches the interesting assertion).
fn setup_picker_state() -> (Owner, Owner, SignalsMap) {
    let parent = Owner::new();
    parent.set();
    let all_signals: SignalsMap = RwSignal::new(BTreeMap::new());

    let child = parent.child();
    child.with(|| {
        let signals: Vec<RwSignal<i32>> =
            vec![RwSignal::new(0), RwSignal::new(0), RwSignal::new(1)];
        let stored: StoredValue<Vec<RwSignal<i32>>> = StoredValue::new(signals);
        all_signals.update(|map| {
            map.insert("Spell Sniper".into(), stored);
        });
    });

    (parent, child, all_signals)
}

/// Submit-walk equivalent of `args_modal::on_submit`: iterate every map
/// entry with non-`try_` `with_value` / `get_untracked`. Panics on a
/// disposed entry — that's the bug.
fn submit_walk(all_signals: SignalsMap) -> BTreeMap<String, Vec<i32>> {
    all_signals.with_untracked(|map| {
        map.iter()
            .map(|(key, stored)| {
                let args = stored.with_value(|signals| {
                    signals.iter().map(|s| s.get_untracked()).collect()
                });
                (key.clone(), args)
            })
            .collect()
    })
}

#[test]
fn unchecked_without_cleanup_panics_on_submit() {
    let (_parent, child, all_signals) = setup_picker_state();

    // Pre-disposal walk reads cleanly — sanity check the harness.
    let pre = submit_walk(all_signals);
    assert_eq!(pre.get("Spell Sniper").map(Vec::as_slice), Some([0, 0, 1].as_slice()));

    // User unchecks → `<Show>` body drops, taking the replacement's
    // signals with it. The pre-fix checkbox handler did NOT remove
    // the map entry, so the next submit panics.
    drop(child);

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = submit_walk(all_signals);
    }))
    .is_err();
    assert!(
        panicked,
        "expected submit-walk over a disposed entry to panic; \
         this is the bug the fix in args_modal.rs prevents."
    );
}

#[test]
fn unchecked_with_cleanup_keeps_submit_safe() {
    let (_parent, child, all_signals) = setup_picker_state();

    // The fix: before the inner view drops (and disposes its signals),
    // remove the picker's registration from the modal-scoped maps.
    // Production calls `clear_replacement_registrations()` from the
    // checkbox `on:change` uncheck branch — equivalent to the inline
    // remove below.
    all_signals.update(|map| {
        map.remove("Spell Sniper");
    });

    drop(child);

    // Submit-walk now sees an empty map and runs without panic. With
    // other (still-alive) entries it would also walk them safely —
    // the contract: "no entry in the map outlives its owning scope."
    let walked = submit_walk(all_signals);
    assert!(
        walked.is_empty(),
        "after cleanup the picker's entry should be gone; got {walked:?}"
    );
}
