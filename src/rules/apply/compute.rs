use std::collections::BTreeMap;

use crate::{
    model::Character,
    rules::{WhenCondition, apply::context::ApplyContext, feature::FeatureDefinition},
};

/// Recompute derived character state. Call after any apply pipeline step
/// that mutates `character.features` so callers can trust the result is
/// finalized.
pub fn compute(character: &mut Character, feat_index: &BTreeMap<Box<str>, FeatureDefinition>) {
    character.compute();
    assign(character, feat_index, WhenCondition::OnCompute);
    character.compute_armor_class();
}

/// Evaluate assignment expressions across all features for the given
/// condition. Each feature runs through an `ApplyContext` keyed by its
/// position in `features.list`; ARG values come from each interactive
/// assignment's matching `feature.inputs[expr_index]`.
pub fn assign(
    character: &mut Character,
    feat_index: &BTreeMap<Box<str>, FeatureDefinition>,
    when: WhenCondition,
) {
    // Snapshot indices upfront so the apply phase can take `&mut character`.
    // Filter features without definitions or without matching assignments to
    // avoid building a context for no-ops.
    let feature_indices: Vec<usize> = character
        .features
        .iter()
        .enumerate()
        .filter_map(|(idx, feature)| {
            let feat_def = feat_index.get(feature.name.as_str())?;
            let assigns = feat_def.assign.as_ref()?;
            assigns
                .iter()
                .any(|assignment| assignment.when == when)
                .then_some(idx)
        })
        .collect();

    for feature_index in feature_indices {
        let Some(feat_def) = feat_index.get(&*character.features.list[feature_index].name) else {
            continue;
        };
        let Some(assignments) = feat_def.assign.as_ref() else {
            continue;
        };
        ApplyContext::new(character, feature_index).run_assignments(assignments, when);
    }
}
