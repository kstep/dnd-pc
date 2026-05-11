use std::{collections::BTreeMap, sync::Arc};

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::args_modal::ArgsModalCtx,
    model::{AssignInputs, Character, CharacterCore, Feature, FeatureSource},
    rules::{
        ApplyInputs, FeaturesView, PendingInputs, RecomputePending, RulesRegistry, WhenCondition,
        apply::{FeatureKey, PendingFeature, cascade},
    },
};

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
                let feat_def = fi.get(&pending_feature.name)?;
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
    base: Option<Arc<CharacterCore>>,
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
/// The full-character re-apply happens later when the user clicks Rebuild;
/// editing one feature should not silently cascade through the rest.
///
/// `placeholder_name` is the feat the modal opens for. `current_name` is the
/// row's actual stored name in `character.features`: when a previous swap
/// landed (e.g. ASI placeholder → Lucky), `placeholder_name == "ASI"` and
/// `current_name == Some("Lucky")`. Pass `None` for `current_name` for non-
/// swap edits (i.e. when the row's name equals the placeholder name).
pub fn edit_inputs_modal(
    store: Store<Character>,
    registry: RulesRegistry,
    placeholder_name: Box<str>,
    source: FeatureSource,
    base: Option<Arc<CharacterCore>>,
    current_name: Option<Box<str>>,
) {
    let current_name = current_name.unwrap_or_else(|| placeholder_name.clone());
    let is_swap = current_name != placeholder_name;

    let pending_input = registry.with_features_index_untracked(|feat_index| {
        let placeholder_def = feat_index.get(&placeholder_name)?;
        let character = store.read_untracked();
        let stored_inputs = character
            .features
            .get_inputs(&current_name, &source)
            .to_vec();

        // Swap-edit: the modal opens for the placeholder, so its own exprs
        // (if any) start empty; the picker is pre-set to the current swap
        // and the swap's stored inputs prefill the picker's exprs per-position.
        // Non-swap: feed the placeholder's exprs directly.
        let (prefill, replacement_prefill, prefilled_replacement) = if is_swap {
            (Vec::new(), stored_inputs, Some(current_name.clone()))
        } else {
            (stored_inputs, Vec::new(), None)
        };

        let mut pending_input = PendingInputs::from_feature(
            placeholder_name.clone(),
            placeholder_def,
            source.clone(),
            WhenCondition::OnFeatureAdd,
            prefill,
            placeholder_def.replace_with,
        )?;
        pending_input.prefilled_replacement = prefilled_replacement;
        pending_input.replacement_prefill = replacement_prefill;
        Some(pending_input)
    });

    let Some(pending_input) = pending_input else {
        return;
    };

    let ctx = expect_context::<ArgsModalCtx>();
    ctx.open(vec![pending_input], base, None, move |inputs| {
        store.update(|character| {
            registry.with_features_index_untracked(|feat_index| {
                let Some(feature) = character.features.find_mut(&current_name, &source) else {
                    return;
                };
                if let Some(old_name) =
                    apply_edit_to_feature(feature, &placeholder_name, &inputs, feat_index)
                {
                    character.features.data_mut().remove(old_name.as_ref());
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
                && let Some(replacement) = prefilled_replacements.get(&*pending_input.feature_name)
            {
                pending_input.prefilled_replacement = Some(replacement.as_str().into());
                if let Some(args) = prefilled.get(replacement) {
                    // Replacement's exprs aren't known here (they depend on
                    // the chosen replacement feat). One-element Vec with
                    // AI-provided args feeds expr 0 only; remaining exprs
                    // render empty. (Old behavior broadcast the same args to
                    // every expr — looked like "AI picked the same skill
                    // twice" for multi-expr replacements; intentional fix.)
                    pending_input.replacement_prefill = vec![AssignInputs {
                        args: args.clone(),
                        dice: Default::default(),
                    }];
                }
            } else if let Some(args) = prefilled.get(&*pending_input.feature_name) {
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

    // Convert AI-seeded name-keyed replacements into FeatureKey-keyed
    // entries by matching each name against the pending list (gives us
    // the source). When a name is absent from pending, the seed is
    // dropped — pending is the authoritative target list.
    let mut seeded_inputs = ApplyInputs::new();
    for pending_feature in &pending {
        if let Some(replacement) = prefilled_replacements.get(&*pending_feature.name) {
            seeded_inputs
                .entry(pending_feature.feature_key())
                .or_default()
                .replacement = Some(replacement.as_str().into());
        }
    }

    if all_inputs.is_empty() {
        apply_batch(store, registry, &pending, &seeded_inputs, &callback);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(all_inputs, None, recompute, move |modal_inputs| {
            // Merge: modal overwrites seeded entries by key (user wins).
            let mut merged = seeded_inputs;
            merged.extend(modal_inputs);
            apply_batch(store, registry, &pending, &merged, &callback);
        });
    }
}

/// Apply a batch of pending features under a single `store.update` +
/// `registry.compute`. Drives `cascade()` with the modal-collected inputs,
/// then invokes the caller's post-apply hook against the now-updated
/// character.
fn apply_batch(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: &[PendingFeature],
    inputs: &ApplyInputs,
    callback: &impl Fn(&mut Character),
) {
    let inputs_for = |key: &FeatureKey| -> Vec<AssignInputs> {
        inputs
            .get(key)
            .map(|input| input.inputs.clone())
            .unwrap_or_default()
    };
    let replacement_for = |key: &FeatureKey| -> Option<Box<str>> {
        inputs
            .get(key)
            .and_then(|input| input.replacement.as_deref().map(Box::from))
    };
    store.update(|character| {
        registry.with_definitions(|caches| {
            registry.with_features_index_untracked(|feat_index| {
                cascade(
                    character,
                    pending,
                    feat_index,
                    caches,
                    &inputs_for,
                    &replacement_for,
                    false,
                );
            });
        });
        callback(character);
        registry.compute(character);
        // Sync labels for any newly-added features so the UI shows
        // localized text immediately, instead of waiting for the next
        // locale-driven layout Effect run.
        registry.fill_from_registry(character);
    });
}

/// Apply the modal's submission to an existing feature in-place.
///
/// `placeholder_name` is the placeholder the modal was opened for (e.g.
/// "ASI"). `feature` is the feature being edited — its current name may
/// equal `placeholder_name` (non-swap edit) or be a previously-picked
/// replacement (swap edit, where `feature.replaces == Some(placeholder_name)`).
///
/// The modal returns its picker choice in
/// `submitted.replacements[placeholder_name]`: `Some(name)` keeps or changes
/// the swap; absent / `None` reverts to the placeholder. New inputs land under
/// `(effective_name, feature.source)`.
///
/// Mutates `feature.{name, category, label, description, replaces, inputs,
/// applied}` and returns the previous name when renamed (caller cleans up
/// `features.data` under that key); returns `None` otherwise.
pub fn apply_edit_to_feature(
    feature: &mut Feature,
    placeholder_name: &str,
    submitted: &ApplyInputs,
    feat_index: FeaturesView<'_>,
) -> Option<Box<str>> {
    let placeholder_key = FeatureKey::new(placeholder_name, feature.source.clone());
    let new_name: Box<str> = submitted
        .get(&placeholder_key)
        .and_then(|input| input.replacement.clone())
        .unwrap_or_else(|| placeholder_name.into());
    let new_replaces = (&*new_name != placeholder_name).then(|| placeholder_name.into());
    let effective_key = FeatureKey::new(&new_name, feature.source.clone());
    let new_inputs = submitted
        .get(&effective_key)
        .map(|input| input.inputs.clone())
        .unwrap_or_default();

    let renamed = new_name != feature.name;
    let dirty = renamed || new_replaces != feature.replaces || new_inputs != feature.inputs;

    let old_name = renamed.then(|| {
        if let Some(new_def) = feat_index.get(&new_name) {
            feature.category = new_def.category;
        }
        // sync_labels repopulates from the new def on the next reactive cycle.
        feature.label = None;
        feature.description = String::new();
        std::mem::replace(&mut feature.name, new_name)
    });
    feature.replaces = new_replaces;
    feature.inputs = new_inputs;
    if dirty {
        feature.applied = false;
    }
    old_name
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use wasm_bindgen_test::*;

    use super::*;
    use crate::{
        model::FeatureCategory,
        rules::{ApplyInput, FeatureDefinition, ReplaceWith},
    };

    fn feat_def(
        name: &str,
        category: FeatureCategory,
        replace_with: ReplaceWith,
    ) -> FeatureDefinition {
        FeatureDefinition {
            name: name.into(),
            stackable: false,
            category,
            replace_with,
            spells: None,
            actions: BTreeMap::new(),
            assign: None,
            prerequisites: None,
        }
    }

    fn make_index(defs: Vec<FeatureDefinition>) -> BTreeMap<Box<str>, FeatureDefinition> {
        defs.into_iter()
            .map(|def| (def.name.clone(), def))
            .collect()
    }

    fn args(values: &[i32]) -> AssignInputs {
        AssignInputs {
            args: values.to_vec(),
            dice: Default::default(),
        }
    }

    fn fighter_l4() -> FeatureSource {
        FeatureSource::Class("Fighter".into(), 4)
    }

    fn feature(
        name: &str,
        category: FeatureCategory,
        source: FeatureSource,
        inputs: Vec<AssignInputs>,
        replaces: Option<Box<str>>,
    ) -> Feature {
        Feature {
            name: name.into(),
            category,
            source,
            inputs,
            replaces,
            applied: true,
            ..Default::default()
        }
    }

    #[wasm_bindgen_test]
    fn non_swap_keeps_inputs_and_stays_applied() {
        let index = make_index(vec![feat_def(
            "Tough",
            FeatureCategory::General,
            ReplaceWith::None,
        )]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let inputs = vec![args(&[1, 2])];
        let mut feature = feature(
            "Tough",
            FeatureCategory::General,
            source.clone(),
            inputs.clone(),
            None,
        );
        let mut submitted = ApplyInputs::new();
        submitted.insert(
            FeatureKey::new("Tough", source.clone()),
            ApplyInput {
                inputs: inputs.clone(),
                replacement: None,
            },
        );

        let renamed_from = apply_edit_to_feature(&mut feature, "Tough", &submitted, view);

        assert_eq!(renamed_from, None);
        assert_eq!(&*feature.name, "Tough");
        assert_eq!(feature.replaces, None);
        assert_eq!(feature.inputs, inputs);
        assert!(feature.applied);
    }

    #[wasm_bindgen_test]
    fn non_swap_inputs_changed_marks_dirty() {
        let index = make_index(vec![feat_def(
            "Tough",
            FeatureCategory::General,
            ReplaceWith::None,
        )]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let mut feature = feature(
            "Tough",
            FeatureCategory::General,
            source.clone(),
            vec![args(&[1, 2])],
            None,
        );
        let new_inputs = vec![args(&[3, 4])];
        let mut submitted = ApplyInputs::new();
        submitted.insert(
            FeatureKey::new("Tough", source.clone()),
            ApplyInput {
                inputs: new_inputs.clone(),
                replacement: None,
            },
        );

        let renamed_from = apply_edit_to_feature(&mut feature, "Tough", &submitted, view);

        assert_eq!(renamed_from, None);
        assert_eq!(feature.inputs, new_inputs);
        assert!(!feature.applied);
    }

    #[wasm_bindgen_test]
    fn swap_picker_unchanged_stays_applied_when_inputs_match() {
        let index = make_index(vec![
            feat_def("ASI", FeatureCategory::Class, ReplaceWith::Any),
            feat_def("Lucky", FeatureCategory::General, ReplaceWith::None),
        ]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let inputs = vec![args(&[7])];
        let mut feature = feature(
            "Lucky",
            FeatureCategory::General,
            source.clone(),
            inputs.clone(),
            Some("ASI".into()),
        );
        let mut submitted = ApplyInputs::new();
        submitted.insert(
            FeatureKey::new("ASI", source.clone()),
            ApplyInput {
                inputs: vec![],
                replacement: Some("Lucky".into()),
            },
        );
        submitted.insert(
            FeatureKey::new("Lucky", source.clone()),
            ApplyInput {
                inputs: inputs.clone(),
                replacement: None,
            },
        );

        let renamed_from = apply_edit_to_feature(&mut feature, "ASI", &submitted, view);

        assert_eq!(renamed_from, None);
        assert_eq!(&*feature.name, "Lucky");
        assert_eq!(feature.replaces, Some("ASI".into()));
        assert!(feature.applied);
    }

    #[wasm_bindgen_test]
    fn swap_picker_changed_renames_and_marks_dirty() {
        let index = make_index(vec![
            feat_def("ASI", FeatureCategory::Class, ReplaceWith::Any),
            feat_def("Lucky", FeatureCategory::General, ReplaceWith::None),
            feat_def("Tough", FeatureCategory::General, ReplaceWith::None),
        ]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let mut feature = feature(
            "Lucky",
            FeatureCategory::General,
            source.clone(),
            vec![args(&[7])],
            Some("ASI".into()),
        );
        let new_inputs = vec![args(&[9])];
        let mut submitted = ApplyInputs::new();
        submitted.insert(
            FeatureKey::new("ASI", source.clone()),
            ApplyInput {
                inputs: vec![],
                replacement: Some("Tough".into()),
            },
        );
        submitted.insert(
            FeatureKey::new("Tough", source.clone()),
            ApplyInput {
                inputs: new_inputs.clone(),
                replacement: None,
            },
        );

        let renamed_from = apply_edit_to_feature(&mut feature, "ASI", &submitted, view);

        assert_eq!(renamed_from, Some("Lucky".into()));
        assert_eq!(&*feature.name, "Tough");
        assert_eq!(feature.replaces, Some("ASI".into()));
        assert_eq!(feature.category, FeatureCategory::General);
        assert_eq!(feature.inputs, new_inputs);
        assert!(!feature.applied);
    }

    #[wasm_bindgen_test]
    fn swap_picker_unchecked_reverts_to_placeholder() {
        let index = make_index(vec![
            feat_def("ASI", FeatureCategory::Class, ReplaceWith::Any),
            feat_def("Lucky", FeatureCategory::General, ReplaceWith::None),
        ]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let mut feature = feature(
            "Lucky",
            FeatureCategory::General,
            source.clone(),
            vec![args(&[7])],
            Some("ASI".into()),
        );
        let placeholder_inputs = vec![args(&[1])];
        let mut submitted = ApplyInputs::new();
        // No replacement — picker unchecked. Inputs land under placeholder.
        submitted.insert(
            FeatureKey::new("ASI", source.clone()),
            ApplyInput {
                inputs: placeholder_inputs.clone(),
                replacement: None,
            },
        );

        let renamed_from = apply_edit_to_feature(&mut feature, "ASI", &submitted, view);

        assert_eq!(renamed_from, Some("Lucky".into()));
        assert_eq!(&*feature.name, "ASI");
        assert_eq!(feature.replaces, None);
        assert_eq!(feature.category, FeatureCategory::Class);
        assert_eq!(feature.inputs, placeholder_inputs);
        assert!(!feature.applied);
    }

    #[wasm_bindgen_test]
    fn swap_changed_uses_replacement_inputs_from_modal() {
        // Inputs under (placeholder, source) must be ignored — the modal
        // submits new inputs under (replacement, source).
        let index = make_index(vec![
            feat_def("ASI", FeatureCategory::Class, ReplaceWith::Any),
            feat_def("Tough", FeatureCategory::General, ReplaceWith::None),
        ]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let mut feature = feature(
            "Lucky",
            FeatureCategory::General,
            source.clone(),
            vec![args(&[7])],
            Some("ASI".into()),
        );
        let mut submitted = ApplyInputs::new();
        submitted.insert(
            FeatureKey::new("ASI", source.clone()),
            ApplyInput {
                inputs: vec![args(&[99])],
                replacement: Some("Tough".into()),
            },
        );
        submitted.insert(
            FeatureKey::new("Tough", source.clone()),
            ApplyInput {
                inputs: vec![args(&[5])],
                replacement: None,
            },
        );

        apply_edit_to_feature(&mut feature, "ASI", &submitted, view);

        assert_eq!(feature.inputs, vec![args(&[5])]);
    }
}
