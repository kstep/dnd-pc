use std::collections::BTreeMap;

use leptos::prelude::*;
use reactive_stores::Store;

use super::context::ArgsContext;
use crate::{
    components::args_modal::ArgsModalCtx,
    model::{AssignInputs, Attribute, Character},
    rules::{
        ApplyInputs, FeatureKey, PendingInputs, ReplaceWith, RulesRegistry, WhenCondition,
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
                let exprs = feat_def.interactive_exprs(WhenCondition::OnLevelUp, &character);
                (!exprs.is_empty()).then_some(PendingInputs {
                    feature_name: feature.name.clone(),
                    feature_label: feat_def.label().to_string(),
                    feature_description: feat_def.description.clone(),
                    exprs,
                    prefill: Vec::new(),
                    replace_with: ReplaceWith::None,
                    source: feature.source.clone(),
                })
            });

        new_inputs.chain(levelup_inputs).collect()
    })
}

/// Top-level helper for the unified feature application pipeline.
/// Collects PendingInputs from pending features, shows the args modal if
/// needed, resolves replacements, calls the user callback, then computes.
pub fn apply_with_modal(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: Vec<PendingFeature>,
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
        ctx.open(all_inputs, move |inputs| apply(Some(&inputs)));
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
        ctx.open(all_inputs, move |inputs| do_replay(Some(&inputs)));
    }
}

/// Like [`apply_with_modal`], but accepts pre-filled ARG values (e.g. from AI
/// generation). Features whose prefilled args validate successfully are applied
/// directly; any remaining features fall back to the interactive args modal.
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
    let all_inputs = collect_all_inputs(&store, &registry, &pending);

    // Partition inputs into pre-filled (validated) and fallback (needs modal)
    let mut validated_inputs = ApplyInputs::default();
    let mut fallback_names: Vec<String> = Vec::new();

    {
        let character = store.read_untracked();
        for pending_input in &all_inputs {
            // Check for prefilled replacement
            if pending_input.is_replaceable()
                && let Some(replacement) = prefilled_replacements.get(&pending_input.feature_name)
            {
                validated_inputs
                    .replacements
                    .insert(pending_input.feature_name.clone(), replacement.clone());
                // If the replacement has ARGs, validate them too
                if let Some(args) = prefilled.get(replacement) {
                    let expr_inputs = pending_input
                        .exprs
                        .iter()
                        .map(|_| AssignInputs {
                            args: args.clone(),
                            dice: Default::default(),
                        })
                        .collect();
                    validated_inputs.feature_inputs.insert(
                        FeatureKey::new(
                            pending_input.feature_name.clone(),
                            pending_input.source.clone(),
                        ),
                        expr_inputs,
                    );
                }
                continue;
            }

            if let Some(args) = prefilled.get(&pending_input.feature_name) {
                let all_valid = pending_input.exprs.iter().all(|expression| {
                    let ctx = ArgsContext {
                        character: &character,
                        args,
                    };
                    expression.eval_lenient(&ctx).is_ok()
                });

                if all_valid {
                    let expr_inputs = pending_input
                        .exprs
                        .iter()
                        .map(|_| AssignInputs {
                            args: args.clone(),
                            dice: Default::default(),
                        })
                        .collect();
                    validated_inputs.feature_inputs.insert(
                        FeatureKey::new(
                            pending_input.feature_name.clone(),
                            pending_input.source.clone(),
                        ),
                        expr_inputs,
                    );
                    continue;
                }
            }
            fallback_names.push(pending_input.feature_name.clone());
        }
    }

    // Features that have interactive assign expressions but weren't in
    // all_inputs (e.g. Expertise whose guard prunes all ARGs before
    // proficiencies are applied) also need to go to fallback.
    let inputs_names: Vec<_> = all_inputs
        .iter()
        .map(|input| input.feature_name.as_str())
        .collect();
    registry.with_features_index_untracked(|fi| {
        for pf in &pending {
            let already_validated = validated_inputs
                .feature_inputs
                .keys()
                .any(|key| key.name == pf.name);
            if !inputs_names.contains(&pf.name.as_str())
                && !already_validated
                && let Some(feat_def) = fi.get(pf.name.as_str())
            {
                let has_args = feat_def.assign.as_ref().is_some_and(|assigns| {
                    assigns.iter().any(|assignment| {
                        assignment
                            .expr
                            .has_var(|var| matches!(var, Attribute::Arg(_)))
                    })
                });
                if has_args {
                    fallback_names.push(pf.name.clone());
                }
            }
        }
    });

    if fallback_names.is_empty() {
        apply_batch(store, registry, &pending, &validated_inputs, &callback);
    } else {
        // Split: apply validated features first, then modal for the rest
        log::debug!("Fallback features needing modal: {fallback_names:?}");

        let validated_pending: Vec<_> = pending
            .iter()
            .filter(|pf| !fallback_names.contains(&pf.name))
            .cloned()
            .collect();
        let fallback_pending: Vec<_> = pending
            .into_iter()
            .filter(|pf| fallback_names.contains(&pf.name))
            .collect();

        apply_batch(
            store,
            registry,
            &validated_pending,
            &validated_inputs,
            &callback,
        );

        // Re-collect inputs for fallback features now that character state
        // has changed (e.g. Expertise needs proficiencies from Class Proficiencies)
        let refreshed_inputs: Vec<PendingInputs> = registry.with_features_index_untracked(|fi| {
            let character = store.read_untracked();
            fallback_pending
                .iter()
                .filter_map(|pending_feature| {
                    let feat_def = fi.get(pending_feature.name.as_str())?;
                    pending_feature.pending_inputs(feat_def, &character)
                })
                .collect()
        });

        // Show modal for remaining features
        if let Some(ctx) = use_context::<ArgsModalCtx>() {
            ctx.open(refreshed_inputs, move |modal_inputs| {
                apply_batch(store, registry, &fallback_pending, &modal_inputs, &callback);
            });
        } else {
            log::warn!("Skipping features without valid ARGs (no modal): {fallback_names:?}");
        }
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
