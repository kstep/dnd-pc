use std::{collections::BTreeMap, sync::Arc};

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::args_modal::ArgsModalCtx,
    model::{AssignInputs, Character, FeatureSource},
    rules::{
        ApplyInputs, PendingInputs, RulesRegistry, WhenCondition,
        apply::{FeatureKey, PendingFeature, replay, resolve_replacements, restore_user_state},
        feature::FeatureDefinition,
    },
};

/// Collect OnFeatureAdd pending inputs from the given pending features list.
pub(super) fn collect_all_inputs(
    store: &Store<Character>,
    registry: &RulesRegistry,
    pending: &[PendingFeature],
) -> Vec<PendingInputs> {
    registry.with_features_index_untracked(|fi| {
        let character = store.read_untracked();
        pending
            .iter()
            .filter_map(|pending_feature| {
                let feat_def = fi.get(pending_feature.name.as_str())?;
                pending_feature.pending_inputs(feat_def, &character)
            })
            .collect()
    })
}

/// Top-level helper for the unified feature application pipeline.
/// Collects PendingInputs from pending features, shows the args modal if
/// needed, resolves replacements, calls the user callback, then computes.
///
/// `base` seeds the cascade snapshot[0]: `None` for the live-store default
/// (level-up / user-add), `Some(character)` for flows that need a custom
/// pre-state (edit mode passes a pre-edit snapshot so analysis doesn't see
/// the feature's own prior contributions).
pub fn apply_with_modal(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: Vec<PendingFeature>,
    base: Option<Arc<Character>>,
    callback: impl Fn(
        &mut Character,
        &[PendingFeature],
        &ApplyInputs,
        &BTreeMap<Box<str>, FeatureDefinition>,
    ) + Send
    + Sync
    + 'static,
) {
    let all_inputs = collect_all_inputs(&store, &registry, &pending);

    let apply = move |inputs: Option<&ApplyInputs>| {
        let empty = ApplyInputs::default();
        let inputs = inputs.unwrap_or(&empty);
        store.update(|character| {
            registry.with_features_index_untracked(|fi| {
                let resolved = resolve_replacements(&pending, &inputs.replacements, fi);
                callback(character, &resolved, inputs, fi);
            });
        });
    };

    if all_inputs.is_empty() {
        apply(None);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(all_inputs, base, move |inputs| apply(Some(&inputs)));
    }
}

/// Edit inputs of an already-applied feature. Opens the args modal with
/// current inputs prefilled against a pre-edit cascade snapshot. On submit,
/// stores the new inputs and marks the feature dirty (`applied = false`).
/// The full-character re-apply happens later when the user clicks Replay;
/// editing one feature should not silently cascade through the rest.
pub fn edit_inputs_modal(
    store: Store<Character>,
    registry: RulesRegistry,
    name: String,
    source: FeatureSource,
    base: Option<Arc<Character>>,
) {
    let pending_input = registry.with_features_index_untracked(|fi| {
        let feat_def = fi.get(name.as_str())?;
        let character = store.read_untracked();
        let prefill = character.features.get_inputs(&name, &source).to_vec();
        PendingInputs::from_feature(
            name.clone(),
            feat_def,
            source.clone(),
            WhenCondition::OnFeatureAdd,
            prefill,
            feat_def.replace_with,
        )
    });

    let Some(pending_input) = pending_input else {
        return;
    };

    let key = FeatureKey::new(name, source);
    let ctx = expect_context::<ArgsModalCtx>();
    ctx.open(vec![pending_input], base, move |inputs| {
        // If the user picked a replacement, rename the feature in-place and
        // pull inputs under the replacement key. `applied = false` only when
        // something actually changed — opening + closing the modal without
        // edits should not trigger a Replay banner.
        let replacement_name = inputs.replacements.get(&key.name).cloned();
        let effective_key = match &replacement_name {
            Some(name) => FeatureKey::new(name.clone(), key.source.clone()),
            None => key.clone(),
        };
        let new_inputs = inputs
            .feature_inputs
            .get(&effective_key)
            .cloned()
            .unwrap_or_default();
        store.update(|character| {
            registry.with_features_index_untracked(|fi| {
                for feature in character.features.iter_mut() {
                    if feature.name == key.name && feature.source == key.source {
                        let renamed = replacement_name
                            .as_ref()
                            .is_some_and(|new_name| new_name != &feature.name);
                        if let Some(new_name) = &replacement_name
                            && let Some(feat_def) = fi.get(new_name.as_str())
                        {
                            feature.name = new_name.clone();
                            feature.label = feat_def.label.clone();
                            feature.description = feat_def.description.clone();
                            feature.category = feat_def.category;
                        }
                        let inputs_changed = feature.inputs != new_inputs;
                        feature.inputs = new_inputs.clone();
                        if renamed || inputs_changed {
                            feature.applied = false;
                        }
                        break;
                    }
                }
                if let Some(new_name) = &replacement_name
                    && new_name != &key.name
                {
                    character.features.data_mut().remove(&key.name);
                }
            });
        });
    });
}

/// Replay all applied features from scratch. Clones the character, resets
/// computed state, collects pending inputs on the clean clone (skipping
/// features with stored inputs), then either replays directly or shows the
/// args modal for features missing stored inputs.
pub fn replay_with_modal(store: Store<Character>, registry: RulesRegistry) {
    let mut clone = store.with_untracked(|character| character.clone());
    clone.reset_computed();

    let mut pending: Vec<PendingFeature> = clone
        .features
        .iter()
        .map(|feature| PendingFeature {
            name: feature.name.clone(),
            source: feature.source.clone(),
            level: feature.source.added_at_level(),
        })
        .collect();
    pending.sort_by_key(|pending_feature| pending_feature.source.added_at_level());

    let mut all_inputs = registry.with_features_index_untracked(|fi| {
        pending
            .iter()
            .filter_map(|pf| {
                let feat_def = fi.get(pf.name.as_str())?;
                pf.pending_inputs(feat_def, &clone)
            })
            .collect::<Vec<_>>()
    });
    // TODO(dead-args): retain drops features that have stored inputs, so
    // replay never opens the modal for them — stored args are passed as-is
    // into `replay()` below. If those stored args contain "dead" positions
    // (non-zero at a slot whose body `if(@ == …)` would no-op under the
    // current baseline — e.g. a pick on a skill another source has since
    // made proficient), apply silently partial-recovers: the live slots
    // take effect, dead ones are ignored. Storage keeps the original
    // (dirty) inputs untouched — no data loss, no crash, but no user
    // notification either.
    //
    // Detection would require per-feature pre-apply baseline via pipeline
    // walk (see prior attempts at `sanitize_stored_inputs`). Using the
    // post-apply `clone` as baseline gives false positives for features
    // that upgrade a slot they themselves touched (e.g. Expertise raises
    // a skill to level 2 — body `if(@ == 1, …)` then looks inactive on
    // re-analyze against current state, flagging legit stored picks as
    // dead). A correct detector needs light apply + staged baseline —
    // deferred until there's a concrete need.
    //
    // Rebuild covers the "data got stale" case explicitly via
    // `simulated.eq_derived(&original)` → modal opens → Effect in
    // `ExprArgsInput` cleans prefill reactively.
    all_inputs.retain(|input| {
        clone
            .features
            .get_inputs(&input.feature_name, &input.source)
            .is_empty()
    });

    let do_replay = move |inputs: Option<&ApplyInputs>| {
        let empty = ApplyInputs::default();
        let inputs = inputs.unwrap_or(&empty);
        store.update(|character| {
            let original_feature_data = character.features.data().clone();
            *character = clone;
            registry.with_features_index_untracked(|fi| {
                replay(fi, character, &pending, inputs);
            });
            restore_user_state(&original_feature_data, character.features.data_mut());
        });
    };

    if all_inputs.is_empty() {
        do_replay(None);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(all_inputs, None, move |inputs| do_replay(Some(&inputs)));
    }
}

/// Like [`apply_with_modal`], but accepts pre-filled ARG values (e.g. from AI
/// generation). All pending features — validated and invalid alike — go through
/// the args modal with their prefill populated. The user can review/edit AI's
/// picks before committing; cancelling leaves the character untouched.
pub fn apply_with_prefilled_args(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: Vec<PendingFeature>,
    prefilled: BTreeMap<String, Vec<i32>>,
    prefilled_replacements: BTreeMap<String, String>,
    callback: impl Fn(
        &mut Character,
        &[PendingFeature],
        &ApplyInputs,
        &BTreeMap<Box<str>, FeatureDefinition>,
    ) + Send
    + Sync
    + 'static,
) {
    // Build PendingInputs for every feature that needs interaction, populating
    // prefill + prefilled_replacement from AI's choices. Cascade snapshots in
    // the modal propagate state correctly, so feature N's analysis sees
    // pending[0..N] applied — no need to commit validated features to the
    // store upfront.
    let all_inputs: Vec<PendingInputs> = collect_all_inputs(&store, &registry, &pending)
        .into_iter()
        .map(|mut pending_input| {
            if pending_input.is_replaceable()
                && let Some(replacement) = prefilled_replacements.get(&pending_input.feature_name)
            {
                pending_input.prefilled_replacement = Some(replacement.clone());
                if let Some(args) = prefilled.get(replacement) {
                    // Replacement's exprs aren't known here (they depend on
                    // the chosen replacement feat). Single AssignInputs with
                    // AI-provided args — ReplacementPicker broadcasts it to
                    // each interactive expr at render time.
                    pending_input.replacement_prefill = Some(AssignInputs {
                        args: args.clone(),
                        dice: Default::default(),
                    });
                }
            } else if let Some(args) = prefilled.get(&pending_input.feature_name) {
                pending_input.prefill = pending_input
                    .exprs
                    .iter()
                    .map(|_| AssignInputs {
                        args: args.clone(),
                        dice: Default::default(),
                    })
                    .collect();
            }
            pending_input
        })
        .collect();

    let seeded_inputs = ApplyInputs {
        feature_inputs: BTreeMap::new(),
        replacements: prefilled_replacements,
    };

    if all_inputs.is_empty() {
        apply_batch(store, registry, &pending, &seeded_inputs, &callback);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(all_inputs, None, move |modal_inputs| {
            // Merge AI-seeded replacements with user-submitted (user wins).
            // `seeded_inputs.feature_inputs` is always empty here — the modal
            // owns all feature_inputs — so this is an assignment rather than
            // a merge.
            let mut merged = seeded_inputs;
            merged.replacements.extend(modal_inputs.replacements);
            merged.feature_inputs = modal_inputs.feature_inputs;
            apply_batch(store, registry, &pending, &merged, &callback);
        });
    }
}

/// Apply a batch of pending features under a single `store.update` +
/// `registry.compute`. Resolves replacements, invokes the caller's callback
/// against the mutable character, then recomputes derived state.
fn apply_batch(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: &[PendingFeature],
    inputs: &ApplyInputs,
    callback: &impl Fn(
        &mut Character,
        &[PendingFeature],
        &ApplyInputs,
        &BTreeMap<Box<str>, FeatureDefinition>,
    ),
) {
    store.update(|character| {
        registry.with_features_index_untracked(|fi| {
            let resolved = resolve_replacements(pending, &inputs.replacements, fi);
            callback(character, &resolved, inputs, fi);
        });
        registry.compute(character);
    });
}
