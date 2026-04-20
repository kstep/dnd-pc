use std::{collections::BTreeMap, sync::Arc};

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::args_modal::ArgsModalCtx,
    model::{AssignInputs, Character},
    rules::{
        ApplyInputs, PendingInputs, ReplaceWith, RulesRegistry, WhenCondition,
        apply::{PendingFeature, replay, resolve_replacements, restore_all_spell_selections},
        feature::FeatureDefinition,
    },
};

/// Collect all pending inputs (OnFeatureAdd for new features + OnLevelUp for
/// existing) from the given pending features list.
pub(super) fn collect_all_inputs(
    store: &Store<Character>,
    registry: &RulesRegistry,
    pending: &[PendingFeature],
) -> Vec<PendingInputs> {
    registry.with_features_index_untracked(|fi| {
        let character = store.read_untracked();

        let new_inputs = pending.iter().filter_map(|pending_feature| {
            let feat_def = fi.get(pending_feature.name.as_str())?;
            pending_feature.pending_inputs(feat_def, &character)
        });

        // Wired for a future OnLevelUp interactive pattern — currently no
        // feature has `@ARG` in OnLevelUp assignments, and onlevelup_pass /
        // reapply_existing pass empty inputs regardless, so these entries
        // reach the modal without a downstream consumer.
        let levelup_inputs = character
            .features
            .iter()
            .filter(|feature| feature.applied)
            .filter_map(|feature| {
                let feat_def = fi.get(feature.name.as_str())?;
                PendingInputs::from_feature(
                    feature.name.clone(),
                    feat_def,
                    feature.source.clone(),
                    WhenCondition::OnLevelUp,
                    Vec::new(),
                    ReplaceWith::None,
                )
            });

        new_inputs.chain(levelup_inputs).collect()
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
            registry.compute(character);
        });
    };

    if all_inputs.is_empty() {
        apply(None);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(all_inputs, base, move |inputs| apply(Some(&inputs)));
    }
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
    all_inputs.retain(|input| clone.features.get_inputs(&input.feature_name).is_empty());

    let do_replay = move |inputs: Option<&ApplyInputs>| {
        let empty = ApplyInputs::default();
        let inputs = inputs.unwrap_or(&empty);
        store.update(|character| {
            let original_feature_data = character.features.data().clone();
            *character = clone;
            registry.with_features_index_untracked(|fi| {
                replay(fi, character, &pending, inputs);
            });
            restore_all_spell_selections(&original_feature_data, character.features.data_mut());
            registry.compute(character);
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
