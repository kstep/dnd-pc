use std::{collections::BTreeMap, sync::Arc};

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::args_modal::ArgsModalCtx,
    model::{AssignInputs, Character, CharacterCore, FeatureCategory, FeatureSource},
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
    placeholder_name: String,
    source: FeatureSource,
    base: Option<Arc<CharacterCore>>,
    current_name: Option<String>,
) {
    let current_name = current_name.unwrap_or_else(|| placeholder_name.clone());
    let is_swap = current_name != placeholder_name;

    let pending_input = registry.with_features_index_untracked(|feat_index| {
        let placeholder_def = feat_index.get(placeholder_name.as_str())?;
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
        // The decision matrix is pure (`compute_edit_outcome`); this closure
        // just threads its result into the store.
        let outcome = registry.with_features_index_untracked(|feat_index| {
            let character = store.read_untracked();
            let current_inputs: Vec<AssignInputs> = character
                .features
                .get_inputs(&current_name, &source)
                .to_vec();
            let current_replaces = character
                .features
                .iter()
                .find(|feature| feature.name == current_name && feature.source == source)
                .and_then(|feature| feature.replaces.clone());
            compute_edit_outcome(
                &placeholder_name,
                &current_name,
                &current_inputs,
                current_replaces.as_deref(),
                &inputs,
                feat_index,
                &source,
            )
        });

        store.update(|character| {
            for feature in character.features.iter_mut() {
                if feature.name == current_name && feature.source == source {
                    if let Some(new_category) = outcome.new_category {
                        feature.name = outcome.new_row_name.clone();
                        feature.category = new_category;
                        // sync_labels repopulates label/description on the
                        // next reactive cycle from the new def.
                        feature.label = None;
                        feature.description = String::new();
                    }
                    feature.replaces = outcome.new_replaces.clone();
                    feature.inputs = outcome.new_inputs.clone();
                    if outcome.dirty {
                        feature.applied = false;
                    }
                    break;
                }
            }
            if current_name != outcome.new_row_name {
                character.features.data_mut().remove(&current_name);
            }
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
        inputs.feature_inputs.get(key).cloned().unwrap_or_default()
    };
    let replacement_for = |name: &str| -> Option<String> { inputs.replacements.get(name).cloned() };
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

/// Outcome of an edit-modal submit. Pure data — produced by
/// [`compute_edit_outcome`] from the modal's `ApplyInputs`, then applied to
/// the store by the modal callback. Keeping the decision matrix in a pure
/// function lets the rename / replaces / dirty rules be unit-tested without
/// Leptos signals.
#[derive(Debug, Clone, PartialEq)]
pub struct EditOutcome {
    pub new_row_name: String,
    pub new_replaces: Option<String>,
    pub new_inputs: Vec<AssignInputs>,
    /// `Some(_)` when the row was renamed and the caller should rewrite the
    /// row's category (and clear locale-cached label/description so
    /// `sync_labels` repopulates from the new def).
    pub new_category: Option<FeatureCategory>,
    /// Set `feature.applied = false` when true — drives the Rebuild banner.
    pub dirty: bool,
}

/// Compute the post-submit row state for the edit modal.
///
/// `placeholder_name` is the placeholder feat the modal was opened for (e.g.
/// "ASI"). `current_name` is the row's actual stored name (e.g. "Lucky" for
/// an existing swap, or "ASI" itself for a non-swap edit). The modal returns
/// its picker choice in `submitted.replacements[placeholder_name]`:
///
/// - `Some(name)` — keep / change the swap; the row ends up named `name`,
///   `replaces = Some(placeholder_name)` if `name != placeholder_name`.
/// - `None` — picker unchecked or never set; the row ends up named
///   `placeholder_name`, `replaces = None`.
pub fn compute_edit_outcome(
    placeholder_name: &str,
    current_name: &str,
    current_inputs: &[AssignInputs],
    current_replaces: Option<&str>,
    submitted: &ApplyInputs,
    feat_index: FeaturesView<'_>,
    source: &FeatureSource,
) -> EditOutcome {
    let replacement_name = submitted.replacements.get(placeholder_name).cloned();
    let new_row_name = replacement_name
        .clone()
        .unwrap_or_else(|| placeholder_name.to_string());
    let new_replaces = (new_row_name != placeholder_name).then(|| placeholder_name.to_string());
    let effective_key = FeatureKey::new(&new_row_name, source.clone());
    let new_inputs = submitted
        .feature_inputs
        .get(&effective_key)
        .cloned()
        .unwrap_or_default();

    let renamed = new_row_name != current_name;
    let new_category = renamed
        .then(|| feat_index.get(new_row_name.as_str()).map(|def| def.category))
        .flatten();
    let replaces_changed = new_replaces.as_deref() != current_replaces;
    let inputs_changed = new_inputs != current_inputs;
    let dirty = renamed || replaces_changed || inputs_changed;

    EditOutcome {
        new_row_name,
        new_replaces,
        new_inputs,
        new_category,
        dirty,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use wasm_bindgen_test::*;

    use super::*;
    use crate::rules::{FeatureDefinition, ReplaceWith};

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

    fn make_index(
        defs: Vec<FeatureDefinition>,
    ) -> BTreeMap<Box<str>, FeatureDefinition> {
        defs.into_iter().map(|def| (def.name.clone(), def)).collect()
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

    #[wasm_bindgen_test]
    fn non_swap_keeps_inputs_and_stays_clean() {
        let index = make_index(vec![feat_def(
            "Tough",
            FeatureCategory::General,
            ReplaceWith::None,
        )]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let current_inputs = vec![args(&[1, 2])];
        let mut submitted = ApplyInputs::default();
        submitted.feature_inputs.insert(
            FeatureKey::new("Tough", source.clone()),
            current_inputs.clone(),
        );

        let outcome = compute_edit_outcome(
            "Tough",
            "Tough",
            &current_inputs,
            None,
            &submitted,
            view,
            &source,
        );

        assert_eq!(outcome.new_row_name, "Tough");
        assert_eq!(outcome.new_replaces, None);
        assert_eq!(outcome.new_inputs, current_inputs);
        assert_eq!(outcome.new_category, None);
        assert!(!outcome.dirty);
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
        let current_inputs = vec![args(&[1, 2])];
        let new_inputs = vec![args(&[3, 4])];
        let mut submitted = ApplyInputs::default();
        submitted.feature_inputs.insert(
            FeatureKey::new("Tough", source.clone()),
            new_inputs.clone(),
        );

        let outcome = compute_edit_outcome(
            "Tough",
            "Tough",
            &current_inputs,
            None,
            &submitted,
            view,
            &source,
        );

        assert!(outcome.dirty);
        assert_eq!(outcome.new_inputs, new_inputs);
    }

    #[wasm_bindgen_test]
    fn swap_picker_unchanged_stays_clean_when_inputs_match() {
        let index = make_index(vec![
            feat_def("ASI", FeatureCategory::Class, ReplaceWith::Any),
            feat_def("Lucky", FeatureCategory::General, ReplaceWith::None),
        ]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let current_inputs = vec![args(&[7])];
        let mut submitted = ApplyInputs::default();
        submitted
            .replacements
            .insert("ASI".into(), "Lucky".into());
        submitted.feature_inputs.insert(
            FeatureKey::new("Lucky", source.clone()),
            current_inputs.clone(),
        );

        let outcome = compute_edit_outcome(
            "ASI",
            "Lucky",
            &current_inputs,
            Some("ASI"),
            &submitted,
            view,
            &source,
        );

        assert_eq!(outcome.new_row_name, "Lucky");
        assert_eq!(outcome.new_replaces, Some("ASI".into()));
        assert_eq!(outcome.new_category, None); // no rename
        assert!(!outcome.dirty);
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
        let current_inputs = vec![args(&[7])];
        let new_inputs = vec![args(&[9])];
        let mut submitted = ApplyInputs::default();
        submitted
            .replacements
            .insert("ASI".into(), "Tough".into());
        submitted.feature_inputs.insert(
            FeatureKey::new("Tough", source.clone()),
            new_inputs.clone(),
        );

        let outcome = compute_edit_outcome(
            "ASI",
            "Lucky",
            &current_inputs,
            Some("ASI"),
            &submitted,
            view,
            &source,
        );

        assert_eq!(outcome.new_row_name, "Tough");
        assert_eq!(outcome.new_replaces, Some("ASI".into()));
        assert_eq!(outcome.new_category, Some(FeatureCategory::General));
        assert_eq!(outcome.new_inputs, new_inputs);
        assert!(outcome.dirty);
    }

    #[wasm_bindgen_test]
    fn swap_picker_unchecked_reverts_to_placeholder() {
        let index = make_index(vec![
            feat_def("ASI", FeatureCategory::Class, ReplaceWith::Any),
            feat_def("Lucky", FeatureCategory::General, ReplaceWith::None),
        ]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let current_inputs = vec![args(&[7])];
        let placeholder_inputs = vec![args(&[1])]; // ASI's own picks
        let mut submitted = ApplyInputs::default();
        // No replacement key — picker unchecked. Inputs land under placeholder.
        submitted.feature_inputs.insert(
            FeatureKey::new("ASI", source.clone()),
            placeholder_inputs.clone(),
        );

        let outcome = compute_edit_outcome(
            "ASI",
            "Lucky",
            &current_inputs,
            Some("ASI"),
            &submitted,
            view,
            &source,
        );

        assert_eq!(outcome.new_row_name, "ASI");
        assert_eq!(outcome.new_replaces, None);
        assert_eq!(outcome.new_category, Some(FeatureCategory::Class));
        assert_eq!(outcome.new_inputs, placeholder_inputs);
        assert!(outcome.dirty);
    }

    #[wasm_bindgen_test]
    fn swap_changed_uses_replacement_inputs_from_modal() {
        // Inputs under (placeholder, source) must be ignored — the modal
        // submits new inputs under (replacement, source). Confirms the
        // effective_key lookup.
        let index = make_index(vec![
            feat_def("ASI", FeatureCategory::Class, ReplaceWith::Any),
            feat_def("Tough", FeatureCategory::General, ReplaceWith::None),
        ]);
        let view = FeaturesView::from_natural(&index);
        let source = fighter_l4();
        let mut submitted = ApplyInputs::default();
        submitted
            .replacements
            .insert("ASI".into(), "Tough".into());
        submitted
            .feature_inputs
            .insert(FeatureKey::new("ASI", source.clone()), vec![args(&[99])]);
        submitted.feature_inputs.insert(
            FeatureKey::new("Tough", source.clone()),
            vec![args(&[5])],
        );

        let outcome = compute_edit_outcome(
            "ASI",
            "Lucky",
            &[args(&[7])],
            Some("ASI"),
            &submitted,
            view,
            &source,
        );

        assert_eq!(outcome.new_inputs, vec![args(&[5])]);
    }
}
