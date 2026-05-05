use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    model::{
        AssignInputs, Character, Feature, FeatureCategory, FeatureData, FeatureSource,
        FeatureValue, IdentitySlot, Spell, SpellData,
    },
    rules::{
        FeaturesView, WhenCondition,
        apply::{
            apply_feature, compute,
            pending::{ApplyInputs, FeatureKey, PendingFeature},
        },
        feature::FeatureDefinition,
    },
};

/// Dry-run helper: push a synthetic feature row into `character.features.list`
/// reflecting `pending` + `inputs`, then run the full apply lifecycle. Used by
/// solver / rebuild dry-run paths that mutate a clone'd character.
pub fn dry_run_apply_feature(
    feat_def: &FeatureDefinition,
    character: &mut Character,
    pending: &PendingFeature,
    inputs: &[AssignInputs],
    when: WhenCondition,
) {
    character.features.list.push(Feature {
        name: pending.name.clone(),
        source: pending.source.clone(),
        applied: true,
        category: feat_def.category,
        inputs: inputs.to_vec(),
        ..Feature::default()
    });
    let feature_index = character.features.list.len() - 1;
    apply_feature(feat_def, character, feature_index, when);
}

/// Resolve replacement choices from modal inputs. For each pending feature
/// that has a replacement mapping, swap it with the replacement feature.
pub fn resolve_replacements(
    pending: &[PendingFeature],
    replacements: &BTreeMap<String, String>,
    features_index: FeaturesView<'_>,
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
    features_index: FeaturesView<'_>,
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
    compute(character, features_index);
}

/// Add a single feature to `character.features` and call
/// `feat.apply(OnFeatureAdd)`. If the feature is already applied, re-apply only
/// updates stored inputs — assignments don't re-run (non-idempotent exprs like
/// `HP.MAX += 5` would double-apply; user triggers Replay to recompute with new
/// inputs).
pub fn apply_new_feature(
    features_index: FeaturesView<'_>,
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
                feature.name.as_str() == &*feat_def.name
                    && feature.applied
                    && (!feat_def.stackable || feature.source == pending_feature.source)
            })
        {
            feature.inputs = inputs.to_vec();
        }
        return;
    }
    // Drop dead placeholders at the same source: any sibling whose
    // `replace_with` selects this feat was waiting to be replaced.
    let pending_source = pending_feature.source.clone();
    let pending_name = pending_feature.name.as_str();
    character.features.list.retain(|other| {
        if other.source != pending_source || other.name == pending_name {
            return true;
        }
        let Some(other_def) = features_index.get(other.name.as_str()) else {
            return true;
        };
        !other_def.replace_with.matches(feat_def)
    });
    character.features.add(
        &pending_feature.name,
        None,
        String::new(),
        feat_def.category,
        pending_feature.source.clone(),
        inputs.to_vec(),
    );
    let feature_index = character.features.list.len() - 1;
    apply_feature(
        feat_def,
        character,
        feature_index,
        WhenCondition::OnFeatureAdd,
    );
}

/// Replay: reset derived state and re-apply all features from stored data.
/// `pending` should be sorted by `added_at_level`. `inputs` supplies
/// supplemental ARG values for features that lack stored inputs.
/// Callers that need to keep user-driven state across the wipe (Choice
/// picks, Points/Die used, prepared spells) snapshot `features.data()`
/// beforehand and call [`restore_user_state`] after replay returns.
pub fn replay(
    features_index: FeaturesView<'_>,
    character: &mut Character,
    pending: &[PendingFeature],
    inputs: &ApplyInputs,
) {
    character.reset_computed();
    // System markers' OnFeatureAdd assigns are accumulators (CLASS.LEVEL +=
    // 1, SPECIES.<name> = 1, etc.). Replay re-runs them on top of the
    // existing identity unless we wipe the fields they re-establish first —
    // otherwise a Sorcerer L12 with six User(1..6) markers becomes L18
    // after replay. Per-marker presence so legacy chars (no markers) keep
    // their identity untouched.
    let mut clear_species = false;
    let mut clear_background = false;
    let mut clear_subclass: BTreeSet<String> = BTreeSet::new();
    let mut clear_class_levels: BTreeSet<String> = BTreeSet::new();
    for feature in &character.features.list {
        match feature.category {
            FeatureCategory::System(IdentitySlot::Species) => clear_species = true,
            FeatureCategory::System(IdentitySlot::Background) => clear_background = true,
            FeatureCategory::System(IdentitySlot::Class) => {
                clear_class_levels.insert(feature.name.clone());
            }
            FeatureCategory::System(IdentitySlot::Subclass) => {
                if let FeatureSource::Class(class_name, _) = &feature.source {
                    clear_subclass.insert(class_name.to_string());
                }
            }
            _ => {}
        }
    }
    if clear_species {
        character.identity.species.clear();
    }
    if clear_background {
        character.identity.background.clear();
    }
    for class_level in character.identity.classes.iter_mut() {
        if clear_class_levels.contains(&class_level.class) {
            class_level.level = 0;
        }
        if clear_subclass.contains(&class_level.class) {
            class_level.subclass = None;
        }
    }
    character
        .identity
        .classes
        .retain(|class_level| class_level.level > 0 || !class_level.class.is_empty());

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
        // Look up the feature's slot in the surviving `features.list`. The
        // pending vector mirrors that list 1:1, but we re-find by (name,
        // source) so out-of-order or missing entries fall through cleanly.
        let Some(feature_index) = character.features.list.iter().position(|feature| {
            feature.name == pending_feature.name && feature.source == pending_feature.source
        }) else {
            log::warn!("replay: pending feature missing from list: {pending_feature:?}");
            continue;
        };
        // Refresh stored inputs in-place so ApplyContext sees them.
        if !feature_inputs.is_empty() {
            character.features.list[feature_index].inputs = feature_inputs.to_vec();
        }
        apply_feature(
            feat_def,
            character,
            feature_index,
            WhenCondition::OnFeatureAdd,
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

    compute(character, features_index);
}

/// Re-apply user-driven state (Choice picks, Points/Die used, prepared
/// spells) from a pre-replay snapshot onto a freshly-applied character.
/// Pure data operation — no compute, no rules eval. Callers like
/// `do_replay` and `merge_preserved` use this after they've finished
/// building the structural shape.
pub fn restore_user_state(
    original_feature_data: &BTreeMap<String, FeatureData>,
    target_feature_data: &mut BTreeMap<String, FeatureData>,
) {
    restore_field_picks(original_feature_data, target_feature_data);
    restore_all_spell_selections(original_feature_data, target_feature_data);
}

/// Build a lean cascade-base character representing the state just before
/// `edited` was added. Used by the args modal's edit flow so the expression
/// analysis sees a pre-edit snapshot (edited feature's own contributions
/// absent), allowing its `@ARG` positions to resolve as if picking fresh.
///
/// If `edited` isn't in the feature list, returns the lean clone unchanged
/// (no truncation, no replay — cascade sees all applied features).
pub fn build_cascade_base_before(
    features_index: FeaturesView<'_>,
    character: &Character,
    edited: &FeatureKey,
) -> Character {
    let mut clone = character.clone_lean();
    if !clone.features.truncate(&edited.name, &edited.source) {
        return clone; // edited not in list — cascade sees the full lean clone
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
    // Two donor queues: cantrips and leveled spells. A leveled donor fits
    // any leveled slot regardless of slot level — the player picks where
    // to put it. Cantrips only feed cantrip slots.
    let (mut cantrip_donors, mut spell_donors): (VecDeque<&Spell>, VecDeque<&Spell>) = original
        .iter()
        .filter(|spell| !spell.sticky && !spell.name.is_empty())
        .partition(|spell| spell.level == 0);

    for slot in target
        .iter_mut()
        .filter(|slot| !slot.sticky && slot.name.is_empty())
    {
        let donor = if slot.level == 0 {
            cantrip_donors.pop_front()
        } else {
            spell_donors.pop_front()
        };
        if let Some(donor) = donor {
            slot.clone_from(donor);
        }
    }
}

/// Restore user-driven field values from `original` into `target` after a
/// fresh apply: Choice picks, Points/Die `used` counters. Definition-driven
/// values (max, die size, choice slot count) come from the fresh apply.
/// Empty Choice slots in `target` are filled from `original` only when the
/// definition still has at least that many slots.
pub fn restore_field_picks(
    original: &BTreeMap<String, FeatureData>,
    target: &mut BTreeMap<String, FeatureData>,
) {
    for (name, target_data) in target.iter_mut() {
        let Some(orig_data) = original.get(name) else {
            continue;
        };
        for target_field in target_data.fields.iter_mut() {
            let Some(orig_field) = orig_data
                .fields
                .iter()
                .find(|orig_field| orig_field.name == target_field.name)
            else {
                continue;
            };
            match (&mut target_field.value, &orig_field.value) {
                (
                    FeatureValue::Points { used, max },
                    FeatureValue::Points {
                        used: orig_used, ..
                    },
                ) => {
                    *used = (*orig_used).min(*max);
                }
                (
                    FeatureValue::Die { used, die },
                    FeatureValue::Die {
                        used: orig_used, ..
                    },
                ) => {
                    *used = (*orig_used).min(die.amount);
                }
                (
                    FeatureValue::Choice { options },
                    FeatureValue::Choice {
                        options: orig_options,
                    },
                ) => {
                    // `zip` caps at the shorter length: extra fresh slots stay
                    // empty for the user, trailing original picks are dropped.
                    // Free-text Choice fields (no catalog) write the value into
                    // `label`, leaving `name` empty — so check both.
                    for (target_opt, orig_opt) in options.iter_mut().zip(orig_options) {
                        let target_empty = target_opt.name.is_empty() && target_opt.label.is_none();
                        let orig_has_value = !orig_opt.name.is_empty() || orig_opt.label.is_some();
                        if target_empty && orig_has_value {
                            *target_opt = orig_opt.clone();
                        }
                    }
                }
                _ => {}
            }
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
