use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::apply::{apply_with_modal, rebuild},
    model::Character,
    rules::{
        RulesRegistry,
        apply::{apply_new_features, collect_pending_features},
    },
};

/// Apply every unapplied class level in one flow. Used by the Build-tab
/// "Apply" hint and the level-up button. Identity is not modified — callers
/// tweak identity (e.g. increment a class level) first and then invoke this
/// to materialize the change.
///
/// If the character requires `rebuild()` (species/background unapplied,
/// stale class in `applied`, lowered level), this delegates to `rebuild`
/// instead — forward apply cannot retract feature contributions, so all
/// drift-back conditions go through the same rebuild flow.
pub fn apply_pending(store: Store<Character>, registry: RulesRegistry) {
    if store.read_untracked().needs_rebuild() {
        rebuild(store, registry);
        return;
    }

    let pending = registry.with_features_index_untracked(|fi| {
        let character = store.read_untracked();
        collect_pending_features(&character, &registry, fi)
    });

    if pending.is_empty() {
        // The level-up grants no new features at this step (e.g. Wizard L6
        // with no subclass picked, where L6 has only subclass-gated
        // features). Still mark the levels applied — otherwise the Build
        // banner sticks around with no way to clear it.
        store.update(mark_all_applied);
        return;
    }

    apply_with_modal(
        store,
        registry,
        pending,
        None,
        move |character, pending, inputs, fi| {
            apply_new_features(fi, character, pending, Some(&inputs.feature_inputs));
            mark_all_applied(character);
        },
    );
}

/// Mark every class-level, plus species/background, as applied. Idempotent:
/// re-marking already-applied levels is a no-op via VecSet. Empty-character
/// case (no features yet → `needs_rebuild` skips this whole branch off) only
/// reaches here, and species/background features come in through
/// `apply_pending` like any other — they need flagging here too, otherwise
/// `BuildPendingApplyHint` keeps insisting they're outstanding.
fn mark_all_applied(character: &mut Character) {
    if !character.identity.species.is_empty() {
        character.applied.species = true;
    }
    if !character.identity.background.is_empty() {
        character.applied.background = true;
    }
    let snapshot: Vec<(String, u32)> = character
        .identity
        .classes
        .iter()
        .filter(|cl| !cl.class.is_empty())
        .map(|cl| (cl.class.clone(), cl.level))
        .collect();
    for (class, max_level) in snapshot {
        for lvl in 1..=max_level {
            character.applied.mark_level(&class, lvl);
        }
    }
}
