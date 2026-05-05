use std::{collections::BTreeMap, sync::Arc};

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::args_modal::ArgsModalCtx,
    model::{AssignInputs, Character, FeatureSource},
    rules::{
        ApplyInputs, FeaturesView, PendingInputs, RecomputePending, RulesRegistry, WhenCondition,
        apply::{
            FeatureKey, PendingFeature, apply_new_features, collect_pending_features,
            resolve_replacements,
        },
    },
};

/// Apply outer `pending` then iterate `collect_pending_features` until no
/// new derived features remain — identity-slot picks chain through several
/// passes (e.g. Class Level → Subclass placeholder → Battle Master subclass
/// features). `MAX_PASSES` caps runaway expansion.
fn apply_pending_cascade(
    character: &mut Character,
    pending: &[PendingFeature],
    inputs: &ApplyInputs,
    registry: &RulesRegistry,
    feat_index: FeaturesView<'_>,
) {
    const MAX_PASSES: usize = 8;

    let resolved = resolve_replacements(pending, &inputs.replacements, feat_index);
    apply_new_features(
        feat_index,
        character,
        &resolved,
        Some(&inputs.feature_inputs),
    );
    for _ in 0..MAX_PASSES {
        let derived = collect_pending_features(character, registry, feat_index);
        if derived.is_empty() {
            return;
        }
        let resolved_derived = resolve_replacements(&derived, &inputs.replacements, feat_index);
        apply_new_features(
            feat_index,
            character,
            &resolved_derived,
            Some(&inputs.feature_inputs),
        );
    }
}

/// Collect OnFeatureAdd pending inputs from the given pending features list.
fn collect_all_inputs(
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
/// the feature's own prior contributions). `recompute` enables speculative
/// cascade — when an identity-slot pick changes mid-modal, the closure runs
/// against a speculative character to recompute the pending list. `None`
/// disables speculation (the modal renders the original `pending` unchanged).
pub fn apply_with_modal(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: Vec<PendingFeature>,
    base: Option<Arc<Character>>,
    recompute: Option<RecomputePending>,
    callback: impl Fn(&mut Character) + Send + Sync + 'static,
) {
    let all_inputs = collect_all_inputs(&store, &registry, &pending);

    let apply = move |inputs: Option<&ApplyInputs>| {
        let empty = ApplyInputs::default();
        let inputs = inputs.unwrap_or(&empty);
        apply_batch(store, registry, &pending, inputs, &callback);
    };

    if all_inputs.is_empty() {
        apply(None);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(all_inputs, base, recompute, move |inputs| {
            apply(Some(&inputs))
        });
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
    // Identity-slot picks (subclass) are committed straight into the live
    // store by the args modal; the rebuild-reasons detector picks up the drift
    // (features.list still carries entries sourced under the prior subclass)
    // and surfaces the rebuild banner — replay would re-run stale entries
    // as-is, which is wrong when the subclass roster itself shifts.
    ctx.open(vec![pending_input], base, None, move |inputs| {
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
                            // Locale-aware label/description: sync_labels fills
                            // them on next reactive cycle.
                            feature.label = None;
                            feature.description = String::new();
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
    recompute: Option<RecomputePending>,
    callback: impl Fn(&mut Character) + Send + Sync + 'static,
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
        ctx.open(all_inputs, None, recompute, move |modal_inputs| {
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
/// `registry.compute`. Runs `apply_pending_cascade` (outer pending +
/// derived features), then invokes the caller's post-apply hook against
/// the now-updated character.
fn apply_batch(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: &[PendingFeature],
    inputs: &ApplyInputs,
    callback: &impl Fn(&mut Character),
) {
    store.update(|character| {
        registry.with_features_index_untracked(|feat_index| {
            apply_pending_cascade(character, pending, inputs, &registry, feat_index);
        });
        callback(character);
        registry.compute(character);
        // Sync labels for any newly-added features so the UI shows
        // localized text immediately, instead of waiting for the next
        // locale-driven layout Effect run.
        registry.fill_from_registry(character);
    });
}
