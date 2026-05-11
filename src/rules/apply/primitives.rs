use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    model::{
        AssignInputs, CharacterCore, Feature, FeatureData, FeatureSource, FeatureValue, Spell,
        SpellData,
    },
    rules::{
        FeaturesView, WhenCondition,
        apply::{
            DefinitionCaches, IdentityChange, apply_feature,
            pending::{FeatureKey, PendingFeature},
        },
        feature::{FeatureDefinition, ReplaceWith},
    },
};

/// Find a replacement feature in `original.features` that fills the
/// placeholder slot identified by `key`. Returns the stored feature's name
/// when:
///
/// 1. `feat_def.replace_with` is not `None`.
/// 2. `(key.name, key.source)` is not already in `original.features` (if it is,
///    the placeholder itself is present and wasn't replaced).
/// 3. Another feature X at the same source has a def matching `replace_with`,
///    isn't in `pending_keys`, and meets prerequisites.
///
/// First match wins. Insertion order in `original.features` is preserved.
pub fn detect_replacement(
    key: &FeatureKey,
    feat_def: &FeatureDefinition,
    original: &CharacterCore,
    feat_index: FeaturesView<'_>,
    pending_keys: &BTreeSet<(&str, &FeatureSource)>,
    baseline: &CharacterCore,
) -> Option<Box<str>> {
    if matches!(feat_def.replace_with, ReplaceWith::None) {
        return None;
    }
    let mut candidate: Option<Box<str>> = None;
    for feature in original
        .features
        .iter()
        .filter(|feature| feature.source == key.source)
    {
        if *feature.name == *key.name {
            return None;
        }
        if pending_keys.contains(&(&*feature.name, &feature.source)) {
            continue;
        }
        if feature
            .replaces
            .as_deref()
            .is_some_and(|name| name == &*key.name)
        {
            return Some(feature.name.clone());
        }
        if candidate.is_none()
            && let Some(candidate_def) = feat_index.get(&feature.name)
            && feat_def.replace_with.matches(candidate_def)
            && candidate_def.meets_prerequisites(baseline)
        {
            candidate = Some(feature.name.clone());
        }
    }
    candidate
}

/// Drain identity changes recorded by `apply_feature`: update `applied`
/// flags and emit class/subclass/species/background features that the new
/// identity slot grants. Caller drains the returned `Vec<PendingFeature>`
/// via `apply_pending` (recursing through `apply_pending`-style logic
/// when needed).
pub fn process_identity_events(
    events: Vec<IdentityChange>,
    character: &mut CharacterCore,
    caches: DefinitionCaches<'_>,
    speculative: bool,
) -> Vec<PendingFeature> {
    // Speculative cascades skip flag stamping so `collect_pending_features`
    // re-surfaces the cascade-discovered class/species/background levels.
    if !speculative {
        apply_identity_flags(&events, character);
    }
    derive_identity_follow_ups(events, character, caches)
}

/// Update `applied` flags from identity events. Real-apply paths run this;
/// the args-modal speculative cascade skips it so `collect_pending_features`
/// doesn't think Wizard L1 is already applied while the user is mid-pick.
pub fn apply_identity_flags(events: &[IdentityChange], character: &mut CharacterCore) {
    for event in events {
        match event {
            IdentityChange::Species(_) => character.applied.species = true,
            IdentityChange::Background(_) => character.applied.background = true,
            IdentityChange::ClassLevel { class, old, new } => {
                for level in (*old + 1)..=*new {
                    character.applied.mark_level(class.as_ref(), level);
                }
            }
            IdentityChange::Subclass { .. } => {}
        }
    }
}

/// Derive the class/subclass/species/background `PendingFeature` follow-ups
/// implied by the identity events, without mutating `applied` flags.
pub fn derive_identity_follow_ups(
    events: Vec<IdentityChange>,
    character: &CharacterCore,
    caches: DefinitionCaches<'_>,
) -> Vec<PendingFeature> {
    let mut follow_ups = Vec::new();

    for event in events {
        match event {
            IdentityChange::Species(name) => {
                let Some(species_def) = caches.species.get(name.as_ref()) else {
                    continue;
                };
                let total_level = character.level().max(1);
                let source = FeatureSource::Species(name.clone());
                for feat_name in &species_def.features {
                    follow_ups.push(PendingFeature {
                        name: feat_name.as_str().into(),
                        source: source.clone(),
                        level: total_level,
                        replaces: None,
                    });
                }
            }
            IdentityChange::Background(name) => {
                let Some(bg_def) = caches.backgrounds.get(name.as_ref()) else {
                    continue;
                };
                let total_level = character.level().max(1);
                let source = FeatureSource::Background(name.clone());
                for feat_name in &bg_def.features {
                    follow_ups.push(PendingFeature {
                        name: feat_name.as_str().into(),
                        source: source.clone(),
                        level: total_level,
                        replaces: None,
                    });
                }
            }
            IdentityChange::ClassLevel { class, old, new } => {
                let Some(class_def) = caches.classes.get(class.as_ref()) else {
                    continue;
                };
                let class_def_name = class_def.name.clone();
                let subclass_name = character
                    .identity
                    .classes
                    .iter()
                    .find(|cl| cl.class.as_ref() == &*class)
                    .and_then(|cl| cl.subclass.clone());
                for level in (old + 1)..=new {
                    if let Some(rules) = class_def.levels.get(&level) {
                        let class_source = FeatureSource::Class(class_def_name.clone(), level);
                        for feat_name in &rules.features {
                            follow_ups.push(PendingFeature {
                                name: feat_name.as_str().into(),
                                source: class_source.clone(),
                                level,
                                replaces: None,
                            });
                        }
                    }
                    if let Some(ref sc) = subclass_name
                        && let Some(subclass_def) = class_def.subclasses.get(sc)
                        && let Some(rules) = subclass_def.levels.get(&level)
                    {
                        let sc_source =
                            FeatureSource::Subclass(class_def_name.clone(), sc.clone(), level);
                        for feat_name in &rules.features {
                            follow_ups.push(PendingFeature {
                                name: feat_name.as_str().into(),
                                source: sc_source.clone(),
                                level,
                                replaces: None,
                            });
                        }
                    }
                }
            }
            IdentityChange::Subclass { class, name } => {
                let Some(class_def) = caches.classes.get(class.as_ref()) else {
                    continue;
                };
                let Some(subclass_def) = class_def.subclasses.get(name.as_ref()) else {
                    continue;
                };
                let class_def_name = class_def.name.clone();
                let class_level = character
                    .identity
                    .classes
                    .iter()
                    .find(|cl| cl.class.as_ref() == &*class)
                    .map(|cl| cl.level)
                    .unwrap_or(0);
                for level in 1..=class_level {
                    if let Some(rules) = subclass_def.levels.get(&level) {
                        let sc_source =
                            FeatureSource::Subclass(class_def_name.clone(), name.clone(), level);
                        for feat_name in &rules.features {
                            follow_ups.push(PendingFeature {
                                name: feat_name.as_str().into(),
                                source: sc_source.clone(),
                                level,
                                replaces: None,
                            });
                        }
                    }
                }
            }
        }
    }

    follow_ups
}

/// Dry-run helper: push a synthetic feature row into `character.features.list`
/// reflecting `pending` + `inputs`, then run the full apply lifecycle. Used by
/// solver / rebuild dry-run paths that mutate a clone'd character.
pub fn dry_run_apply_feature(
    feat_def: &FeatureDefinition,
    character: &mut CharacterCore,
    pending: &PendingFeature,
    inputs: &[AssignInputs],
    when: WhenCondition,
    caches: DefinitionCaches<'_>,
) -> Vec<PendingFeature> {
    let feature_pos = character.features.push(Feature {
        name: pending.name.clone(),
        source: pending.source.clone(),
        applied: true,
        category: feat_def.category,
        inputs: inputs.to_vec(),
        ..Feature::default()
    });
    let events = apply_feature(feat_def, character, feature_pos, when);
    process_identity_events(events, character, caches, false)
}

/// Apply `pending` to `character`, recursing on identity-driven follow-ups
/// until the cascade settles. Replacements (placeholder → user-picked feat)
/// and inputs are pulled lazily through closures so the source stays native
/// (frozen map / reactive signals / stored `feature.inputs`) — no eager
/// materialization. Missing-def follow-ups warn and silently skip.
#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "apply.cascade", skip_all, fields(n = pending.len()))
)]
pub fn cascade(
    character: &mut CharacterCore,
    pending: &[PendingFeature],
    features_index: FeaturesView<'_>,
    caches: DefinitionCaches<'_>,
    inputs_for: &dyn Fn(&FeatureKey) -> Vec<AssignInputs>,
    replacement_for: &dyn Fn(&FeatureKey) -> Option<Box<str>>,
    speculative: bool,
) {
    let resolved: Vec<PendingFeature> = pending
        .iter()
        .map(|pending_feature| {
            let key = FeatureKey::from_pending(pending_feature);
            match replacement_for(&key) {
                Some(replacement) if features_index.contains_key(&replacement) => {
                    // Pending's own `replaces` (set by plan for marker-direct
                    // emission) wins; otherwise derive from name mismatch.
                    let replaces = pending_feature.replaces.clone().or_else(|| {
                        (replacement != pending_feature.name).then(|| pending_feature.name.clone())
                    });
                    PendingFeature {
                        name: replacement,
                        source: pending_feature.source.clone(),
                        level: pending_feature.level,
                        replaces,
                    }
                }
                Some(replacement) => {
                    log::warn!(
                        "cascade: replacement '{replacement}' for '{}' not found in index — \
                         keeping original",
                        pending_feature.name
                    );
                    pending_feature.clone()
                }
                None => pending_feature.clone(),
            }
        })
        .collect();

    let mut follow_ups = Vec::new();
    for pending_feature in &resolved {
        let key = FeatureKey::from_pending(pending_feature);
        let inputs = inputs_for(&key);
        let was_applied_before = character
            .features
            .find_pos(&pending_feature.name, &pending_feature.source)
            .is_some_and(|pos| character.features.list[pos].applied);
        let fu = apply_pending(
            features_index,
            character,
            pending_feature,
            &inputs,
            caches,
            speculative,
        );
        if !was_applied_before
            && let Some(replaced_name) = &pending_feature.replaces
            && let Some(pos) = character
                .features
                .find_pos(&pending_feature.name, &pending_feature.source)
        {
            character.features.list[pos].replaces = Some(replaced_name.clone());
        }
        follow_ups.extend(fu);
    }

    if !follow_ups.is_empty() {
        cascade(
            character,
            &follow_ups,
            features_index,
            caches,
            inputs_for,
            replacement_for,
            speculative,
        );
    }
}

/// Add a single feature to `character.features` and call
/// `feat.apply(OnFeatureAdd)`. If the feature is already applied, re-apply only
/// updates stored inputs — assignments don't re-run (non-idempotent exprs like
/// `HP.MAX += 5` would double-apply; user triggers Replay to recompute with new
/// inputs).
pub fn apply_pending(
    features_index: FeaturesView<'_>,
    character: &mut CharacterCore,
    pending_feature: &PendingFeature,
    inputs: &[AssignInputs],
    caches: DefinitionCaches<'_>,
    speculative: bool,
) -> Vec<PendingFeature> {
    let Some(feat_def) = features_index.get(&pending_feature.name) else {
        log::warn!("apply_pending: skipping feature with no definition: {pending_feature:?}");
        return Vec::new();
    };
    if character
        .features
        .contains(&feat_def.name, feat_def.stackable, &pending_feature.source)
    {
        if !inputs.is_empty()
            && let Some(feature) = character.features.iter_mut().find(|feature| {
                *feature.name == *feat_def.name
                    && feature.applied
                    && (!feat_def.stackable || feature.source == pending_feature.source)
            })
        {
            feature.inputs = inputs.to_vec();
        }
        return Vec::new();
    }
    // Drop dead placeholders at the same source: any sibling whose
    // `replace_with` selects this feat was waiting to be replaced.
    character.features.list.retain(|other| {
        if other.source != pending_feature.source || other.name.as_ref() == &*pending_feature.name {
            return true;
        }
        let Some(other_def) = features_index.get(&other.name) else {
            return true;
        };
        !other_def.replace_with.matches(feat_def)
    });

    let inputs_sufficient = feat_def.inputs_sufficient(inputs);
    // Speculative cascade keeps everything as placeholders so the modal's
    // recompute / chain re-collect cleanly via the same `applied=false` path.
    let applied = !speculative && inputs_sufficient;
    let feature_pos = character.features.put(
        &pending_feature.name,
        None,
        String::new(),
        feat_def.category,
        pending_feature.source.clone(),
        inputs.to_vec(),
        applied,
    );
    if !inputs_sufficient {
        log::info!(
            "apply_pending: {} pushed applied=false (inputs insufficient); modal will collect on next open",
            pending_feature.name
        );
        return Vec::new();
    }

    let events = apply_feature(
        feat_def,
        character,
        feature_pos,
        WhenCondition::OnFeatureAdd,
    );
    process_identity_events(events, character, caches, speculative)
}

/// Re-apply user-driven state (Choice picks, Points/Die used, prepared
/// spells) from a pre-replay snapshot onto a freshly-applied character.
/// Pure data operation — no compute, no rules eval. Callers like
/// `merge_preserved` use this after they've finished building the
/// structural shape.
pub fn restore_user_state(
    original_feature_data: &BTreeMap<Box<str>, FeatureData>,
    target_feature_data: &mut BTreeMap<Box<str>, FeatureData>,
) {
    restore_field_picks(original_feature_data, target_feature_data);
    restore_all_spell_selections(original_feature_data, target_feature_data);
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
    original: &BTreeMap<Box<str>, FeatureData>,
    target: &mut BTreeMap<Box<str>, FeatureData>,
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
    original_feature_data: &BTreeMap<Box<str>, FeatureData>,
    target_feature_data: &mut BTreeMap<Box<str>, FeatureData>,
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
