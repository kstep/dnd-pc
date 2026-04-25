use std::collections::{BTreeMap, VecDeque};

use crate::{
    model::{AssignInputs, Character, FeatureData, Spell, SpellData},
    rules::{
        WhenCondition,
        apply::pending::{ApplyInputs, FeatureKey, PendingFeature},
        feature::FeatureDefinition,
    },
};

/// Resolve replacement choices from modal inputs. For each pending feature
/// that has a replacement mapping, swap it with the replacement feature.
pub fn resolve_replacements(
    pending: &[PendingFeature],
    replacements: &BTreeMap<String, String>,
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
) -> Vec<PendingFeature> {
    if replacements.is_empty() {
        return pending.to_vec();
    }
    pending
        .iter()
        .map(|pending_feature| {
            if let Some(replacement_name) = replacements.get(&pending_feature.name) {
                if features_index.contains_key(replacement_name.as_str()) {
                    PendingFeature {
                        name: replacement_name.clone(),
                        source: pending_feature.source.clone(),
                        level: pending_feature.level,
                    }
                } else {
                    log::warn!("Replacement feature '{replacement_name}' not found in index");
                    pending_feature.clone()
                }
            } else {
                pending_feature.clone()
            }
        })
        .collect()
}

/// Batch variant of `apply_new_feature`. Iterates `pending`, looking up each
/// feature's inputs from the map by `FeatureKey`; features not present in the
/// map apply with empty inputs.
#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "apply.new_features", skip_all, fields(n = pending.len()))
)]
pub fn apply_new_features(
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
    character: &mut Character,
    pending: &[PendingFeature],
    feature_inputs: Option<&BTreeMap<FeatureKey, Vec<AssignInputs>>>,
) {
    for pending_feature in pending {
        let key = FeatureKey::from_pending(pending_feature);
        let inputs = feature_inputs
            .and_then(|map| map.get(&key))
            .map(Vec::as_slice)
            .unwrap_or_default();
        apply_new_feature(features_index, character, pending_feature, inputs);
    }
}

/// Add a single feature to `character.features` and call
/// `feat.apply(OnFeatureAdd)`. If the feature is already applied, re-apply only
/// updates stored inputs — assignments don't re-run (non-idempotent exprs like
/// `MAX_HP += 5` would double-apply; user triggers Replay to recompute with new
/// inputs).
pub fn apply_new_feature(
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
    character: &mut Character,
    pending_feature: &PendingFeature,
    inputs: &[AssignInputs],
) {
    let Some(feat_def) = features_index.get(pending_feature.name.as_str()) else {
        log::warn!("apply_new_feature: skipping feature with no definition: {pending_feature:?}");
        return;
    };
    if character
        .features
        .contains(&feat_def.name, feat_def.stackable, &pending_feature.source)
    {
        if !inputs.is_empty()
            && let Some(feature) = character.features.iter_mut().find(|feature| {
                feature.name == feat_def.name
                    && feature.applied
                    && (!feat_def.stackable || feature.source == pending_feature.source)
            })
        {
            feature.inputs = inputs.to_vec();
        }
        return;
    }
    character.features.add(
        &pending_feature.name,
        feat_def.label.clone(),
        feat_def.description.clone(),
        feat_def.category,
        pending_feature.source.clone(),
        inputs.to_vec(),
    );
    feat_def.apply(
        pending_feature.level,
        character,
        WhenCondition::OnFeatureAdd,
        inputs,
    );
}

/// Replay: reset derived state and re-apply all features from stored data.
/// `pending` should be sorted by `added_at_level`. `inputs` supplies
/// supplemental ARG values for features that lack stored inputs.
pub fn replay(
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
    character: &mut Character,
    pending: &[PendingFeature],
    inputs: &ApplyInputs,
) {
    character.reset_computed();

    // Phase 1: OnFeatureAdd at added_at_level.
    // Collect stored inputs upfront to avoid borrow conflict with
    // def.apply() which takes &mut Character.
    let stored_inputs: Vec<_> = pending
        .iter()
        .map(|pending_feature| {
            character
                .features
                .get_inputs(&pending_feature.name, &pending_feature.source)
                .to_vec()
        })
        .collect();
    for (pending_feature, stored) in pending.iter().zip(&stored_inputs) {
        let Some(feat_def) = features_index.get(pending_feature.name.as_str()) else {
            log::warn!("replay: skipping feature with no definition: {pending_feature:?}");
            continue;
        };
        let feature_inputs = if stored.is_empty() {
            inputs.get(&pending_feature.name, &pending_feature.source)
        } else {
            stored.as_slice()
        };
        feat_def.apply(
            pending_feature.level,
            character,
            WhenCondition::OnFeatureAdd,
            feature_inputs,
        );
    }

    // Mark all features as applied and persist supplemental inputs
    for feature in character.features.iter_mut() {
        feature.applied = true;
        if feature.inputs.is_empty() {
            let supp = inputs.get(&feature.name, &feature.source);
            if !supp.is_empty() {
                feature.inputs = supp.to_vec();
            }
        }
    }
}

/// Build a lean cascade-base character representing the state just before
/// `edited` was added. Used by the args modal's edit flow so the expression
/// analysis sees a pre-edit snapshot (edited feature's own contributions
/// absent), allowing its `@ARG` positions to resolve as if picking fresh.
///
/// If `edited` isn't in the feature list, returns the lean clone unchanged
/// (no truncation, no replay — cascade sees all applied features).
pub fn build_cascade_base_before(
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
    character: &Character,
    edited: &FeatureKey,
) -> Character {
    let mut clone = character.clone_lean();
    if !clone.features.truncate(&edited.name, &edited.source) {
        return clone; // edited не найден в list — cascade видит весь lean-клон
    }
    let pending: Vec<PendingFeature> = clone
        .features
        .iter()
        .map(|feature| PendingFeature {
            name: feature.name.clone(),
            source: feature.source.clone(),
            level: feature.source.added_at_level(),
        })
        .collect();
    replay(
        features_index,
        &mut clone,
        &pending,
        &ApplyInputs::default(),
    );
    clone
}

/// Restore user spell selections from original SpellData into replayed target.
/// Matches by level: for each empty non-sticky slot in target, takes the next
/// named spell of the same level from original.
fn restore_spell_selections(original: &SpellData, target: &mut SpellData) {
    restore_spell_list(&original.spells, &mut target.spells);
    if let (Some(orig_known), Some(target_known)) = (&original.known, target.known.as_mut()) {
        restore_spell_list(orig_known, target_known);
    }
}

fn restore_spell_list(original: &[Spell], target: &mut [Spell]) {
    let mut by_level: BTreeMap<u32, VecDeque<&Spell>> = original
        .iter()
        .filter(|spell| !spell.sticky && !spell.name.is_empty())
        .fold(BTreeMap::new(), |mut map, spell| {
            map.entry(spell.level).or_default().push_back(spell);
            map
        });

    for slot in target
        .iter_mut()
        .filter(|slot| !slot.sticky && slot.name.is_empty())
    {
        if let Some(donor) = by_level.get_mut(&slot.level).and_then(VecDeque::pop_front) {
            slot.name = donor.name.clone();
            slot.label = donor.label.clone();
            slot.description = donor.description.clone();
        }
    }
}

/// Restore user-selected spells from `original` feature_data into `clean`.
/// Iterates entries present in both, calling `restore_spell_selections` for
/// matching spells blocks. Shared by rebuild's `merge_preserved` and replay.
pub fn restore_all_spell_selections(
    original_feature_data: &BTreeMap<String, FeatureData>,
    target_feature_data: &mut BTreeMap<String, FeatureData>,
) {
    for (name, original_data) in original_feature_data {
        if let (Some(original_spells), Some(target_spells)) = (
            &original_data.spells,
            target_feature_data
                .get_mut(name)
                .and_then(|data| data.spells.as_mut()),
        ) {
            restore_spell_selections(original_spells, target_spells);
        }
    }
}
