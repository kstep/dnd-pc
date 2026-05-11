use crate::{
    model::{Character, CharacterCore},
    rules::{
        FeaturesView, WhenCondition,
        apply::{context::ApplyContext, item_ctx::assign_items},
    },
};

/// Recompute derived character state. Call after any apply pipeline step
/// that mutates `character.features` so callers can trust the result is
/// finalized. Takes `Character` (not `CharacterCore`) because
/// `compute_armor_class` reads `equipment.armors`.
pub fn compute(character: &mut Character, feat_index: FeaturesView<'_>) {
    character.compute();
    assign(character, feat_index, WhenCondition::OnCompute);
    assign_items(character, WhenCondition::OnGearActive);
    character.compute_armor_class();
}

/// Core-only variant of `compute` for speculative cascade previews. Skips
/// `compute_armor_class` (equipment-dependent); previews don't render AC.
pub fn compute_core(character: &mut CharacterCore, feat_index: FeaturesView<'_>) {
    character.compute();
    assign(character, feat_index, WhenCondition::OnCompute);
}

/// Evaluate assignment expressions across all features for the given
/// condition. Each feature runs through an `ApplyContext` keyed by its
/// position in `features.list`; ARG values come from each interactive
/// assignment's matching `feature.inputs[expr_index]`.
pub fn assign(character: &mut CharacterCore, feat_index: FeaturesView<'_>, when: WhenCondition) {
    // Snapshot indices upfront so the apply phase can take `&mut character`.
    // Filter features without definitions or without matching assignments to
    // avoid building a context for no-ops.
    let feature_positions: Vec<usize> = character
        .features
        .iter()
        .enumerate()
        .filter_map(|(idx, feature)| {
            let feat_def = feat_index.get(&feature.name)?;
            let assigns = feat_def.assign.as_ref()?;
            assigns
                .iter()
                .any(|assignment| assignment.when == when)
                .then_some(idx)
        })
        .collect();

    for feature_pos in feature_positions {
        let Some(feat_def) = feat_index.get(&character.features.list[feature_pos].name) else {
            continue;
        };
        let Some(assignments) = feat_def.assign.as_ref() else {
            continue;
        };
        ApplyContext::new(character, feature_pos).run_assignments(assignments, when);
    }
}
