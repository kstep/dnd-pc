//! Reproduces the bug where unchecking the replacement picker after a
//! prior selection leaves stale `StoredValue<Vec<RwSignal<i32>>>` entries
//! in the modal's `all_signals` map. On submit, the modal walks every
//! map entry and calls `signal.get_untracked()` on the inner signals.
//! Those signals were created in the now-disposed reactive scope of the
//! `<Show>` body that the uncheck unmounted, and the access becomes
//! unsafe.
//!
//! The test models the lifecycle directly: a child Owner scope creates
//! signals, a parent map records references, the child Owner is
//! dropped, the parent then walks the map. It confirms leptos disposes
//! the inner signals when the child Owner drops, which matches the
//! user-reported "first click does nothing, second click reloads the
//! page" symptom (panic in `on_submit` aborts the modal handler before
//! `ctx.show.set(false)` can close it; the WASM panic hook poisons
//! subsequent event handling).

use std::collections::BTreeMap;

use leptos::prelude::*;

#[test]
fn child_scope_signal_after_owner_drop_is_disposed() {
    // Parent owner — analogous to ArgsModal's component scope which owns
    // the long-lived `all_signals` map.
    let parent = Owner::new();
    parent.set();

    // Mirror of `state.args` (RwSignal<BTreeMap<FeatureKey,
    // Vec<StoredValue<Vec<RwSignal<i32>>>>>>) — collapsed to a single
    // signal vec keyed by string for the test.
    let all_signals: RwSignal<BTreeMap<String, StoredValue<Vec<RwSignal<i32>>>>> =
        RwSignal::new(BTreeMap::new());

    // Child owner — analogous to the `<Show when=replacing>` body in
    // ReplacementPicker. The replacement's ExprArgsInput is mounted
    // inside it; its RwSignals are owned by it.
    let child = parent.child();
    child.with(|| {
        let signals: Vec<RwSignal<i32>> = vec![RwSignal::new(0), RwSignal::new(0), RwSignal::new(1)];
        let stored: StoredValue<Vec<RwSignal<i32>>> = StoredValue::new(signals);
        // The modal stores the StoredValue ref under the replacement's
        // (name, source) key.
        all_signals.update(|map| {
            map.insert("Spell Sniper".to_string(), stored);
        });
    });

    // While child is alive the signals read fine.
    all_signals.with_untracked(|map| {
        let stored = map.get("Spell Sniper").expect("entry present");
        stored.with_value(|signals| {
            let collected: Vec<i32> = signals.iter().map(|s| s.get_untracked()).collect();
            assert_eq!(collected, vec![0, 0, 1]);
        });
    });

    // User unchecks the replacement → `<Show>` content unmounts, child
    // Owner is dropped. Modal's checkbox handler does NOT remove the map
    // entry, so the stale StoredValue stays in `all_signals`.
    drop(child);

    // Submit-walk in args_modal::on_submit calls `sigs.with_value(...)`
    // and `signal.get_untracked()` on every map entry. After child drop,
    // both the StoredValue and its inner RwSignals are inaccessible.
    // Probe with the `try_*` variants so the test itself doesn't panic
    // — the production code uses non-`try_*` and would panic / return
    // garbage here.
    let stored_alive: bool;
    let inner_alive: Vec<Option<i32>>;
    {
        let probe = all_signals.with_untracked(|map| {
            map.get("Spell Sniper")
                .copied()
                .expect("stale entry remains in map")
        });
        stored_alive = probe.try_with_value(|_| ()).is_some();
        inner_alive = probe
            .try_with_value(|signals| {
                signals.iter().map(|s| s.try_get_untracked()).collect()
            })
            .unwrap_or_default();
    }

    println!(
        "stored_alive={stored_alive}, inner_alive={inner_alive:?}",
    );
    assert!(
        !stored_alive || inner_alive.iter().any(|opt| opt.is_none()),
        "after child Owner drop, expected the StoredValue or its inner \
         RwSignals to be disposed; got stored_alive={stored_alive} \
         inner_alive={inner_alive:?}. If everything is still readable, \
         the disposal-on-scope-drop hypothesis is wrong."
    );

    // The production submit handler in `args_modal.rs:940` calls the
    // non-`try_` `sigs.with_value(...)` on every map entry. Confirm it
    // panics on the disposed entry — that's the actual user-visible
    // failure mode (modal handler aborts mid-submit; on the next click
    // the form HTTP-submits because the wasm runtime is poisoned).
    let probe = all_signals.with_untracked(|map| {
        map.get("Spell Sniper")
            .copied()
            .expect("stale entry remains in map")
    });
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        probe.with_value(|signals| {
            signals
                .iter()
                .map(|s| s.get_untracked())
                .collect::<Vec<_>>()
        })
    }))
    .is_err();
    assert!(
        panicked,
        "expected `StoredValue::with_value` on a disposed entry to panic — \
         that's exactly what happens in args_modal::on_submit when the user \
         unchecks the replacement and then submits."
    );
}
