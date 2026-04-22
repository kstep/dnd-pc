use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::ReadUntracked;
use strum::VariantArray;

use crate::{
    model::{
        Ability, Applied, AssignInputs, Attribute, Character, Feature, FeatureCategory,
        FeatureSource, FeatureValue,
    },
    rules::{
        ClassDefinition, ClassIndexEntry, DefinitionStore, RulesRegistry, WhenCondition,
        apply::{
            collect::{
                collect_background_features, collect_class_features, collect_pending_features,
                collect_species_features,
            },
            pending::{ApplyInputs, FeatureKey, PendingFeature, PendingInputs},
            primitives::{
                apply_new_feature, onlevelup_pass, resolve_replacements,
                restore_all_spell_selections,
            },
            reconcile::reconcile_user_feature_sources,
            solver::{AssignData, FeatState, outer_group, scan_arg_range, solve_all},
        },
        feature::{FeatureDefinition, ReplaceWith},
    },
};

#[derive(Debug, Clone)]
pub enum RebuildError {
    MissingDefinition { kind: &'static str, name: String },
    MulticlassPrereq { class: String },
}

/// Snapshot-level step 1: reconcile User-sourced features against identity
/// slots, then collect pending inputs that need user interaction. The returned
/// `Character` is the reconciled snapshot ready to feed into `build_clean`.
pub fn prepare_rebuild(
    mut original: Character,
    registry: &RulesRegistry,
) -> (Character, Vec<PendingInputs>) {
    reconcile_user_feature_sources(&mut original, registry);
    let pending_inputs = collect_rebuild_pending_inputs(&original, registry);
    (original, pending_inputs)
}

/// Snapshot-level step 2: build a fresh `Character` from `default()`,
/// applying identity + User features in canonical order (User(0) → Species →
/// Background → classes round-robin with multiclass prereq gates), running
/// `onlevelup_pass`, then merging preserved user state (HP, used counters,
/// spell selections, equipment, personality, notes). Fails if a class /
/// species / background definition is missing from the registry caches or if
/// a multiclass prereq never passes during the build.
pub fn build_clean(
    original: &Character,
    registry: &RulesRegistry,
    extra_inputs: &ApplyInputs,
) -> Result<Character, RebuildError> {
    let mut clean = Character::from_identity(original.identity.clone());

    registry.with_features_index_untracked(|fi| -> Result<(), RebuildError> {
        // 1. User(0) features (e.g. Generation: * setting base abilities)
        apply_user_features_at_level(original, &mut clean, fi, 0, extra_inputs)?;

        // 2. Species
        if !clean.identity.species.is_empty() {
            let species_cache = registry.species().cache().read_untracked();
            let species_def = species_cache
                .get(clean.identity.species.as_str())
                .ok_or_else(|| RebuildError::MissingDefinition {
                    kind: "species",
                    name: clean.identity.species.clone(),
                })?;
            let pending: Vec<PendingFeature> =
                collect_species_features(&clean, species_def, fi).collect();
            apply_pending(fi, &mut clean, &pending, original, extra_inputs)?;
            clean.applied.species = true;
        }

        // 3. Background
        if !clean.identity.background.is_empty() {
            let bg_cache = registry.backgrounds().cache().read_untracked();
            let bg_def = bg_cache
                .get(clean.identity.background.as_str())
                .ok_or_else(|| RebuildError::MissingDefinition {
                    kind: "background",
                    name: clean.identity.background.clone(),
                })?;
            let pending: Vec<PendingFeature> =
                collect_background_features(&clean, bg_def, fi).collect();
            apply_pending(fi, &mut clean, &pending, original, extra_inputs)?;
            clean.applied.background = true;
        }

        // 4. Classes — interleave class levels with per-step prereq filter for
        //    multiclasses (see `apply_classes_interleaved` for details).
        apply_classes_interleaved(registry, fi, &mut clean, original, extra_inputs)?;

        // 5. OnLevelUp sweep for all applied features.
        onlevelup_pass(fi, &mut clean);

        // 6. Legacy migration: characters built before the Generation-feature system
        //    have abilities edited directly. Convert those custom scores into a
        //    synthesized `Generation: Custom` feature so future rebuilds keep them
        //    intact.
        migrate_legacy_abilities(&mut clean, original);

        Ok(())
    })?;

    merge_preserved(&mut clean, original);
    Ok(clean)
}

/// Collect features scheduled for the rebuild that still need user input
/// (OnFeatureAdd interactive exprs and no stored inputs in the original).
/// Covers both User features already in `original.features` and identity
/// features (species/background/classes) that will be added during rebuild.
fn collect_rebuild_pending_inputs(
    original: &Character,
    registry: &RulesRegistry,
) -> Vec<PendingInputs> {
    registry.with_features_index_untracked(|fi| {
        // Identity pending: what `collect_pending_features` would add if no
        // identity feature were yet applied. Snapshot keeps only User features
        // and resets applied flags so the collect functions see a clean slate.
        // Full Character clone is wasteful (most fields unused by collect) but
        // this runs once per Rebuild button click — not worth the mem::replace
        // gymnastics to save ~20µs on a user action.
        let mut snapshot = original.clone();
        snapshot
            .features
            .list
            .retain(|feature| feature.source.is_user());
        snapshot.applied = Applied::default();
        let identity_pending = collect_pending_features(&snapshot, registry, fi);

        let user_pending = original
            .features
            .iter()
            .filter(|feature| feature.source.is_user())
            .map(|feature| PendingFeature {
                name: feature.name.clone(),
                source: feature.source.clone(),
                level: feature.source.added_at_level(),
            });

        let all_pending: Vec<PendingFeature> = user_pending.chain(identity_pending).collect();

        // Precompute keys of every pending (name, source) so
        // `detect_replacement` can tell "not in the rebuilt list" from "in
        // the list under a different slot". Borrows from `all_pending` which
        // outlives this closure's inputs list.
        let pending_keys: BTreeSet<(&str, &FeatureSource)> = all_pending
            .iter()
            .map(|pending| (pending.name.as_str(), &pending.source))
            .collect();

        // Build FeatState for every pending — stored-input feats go in
        // too, their interactive assigns marked `forced` with the stored
        // args so enumerate_assign yields that single candidate. Keeping
        // stored and unsolved in one list preserves pipeline order: e.g.
        // an Expertise-style feat whose `if(@==1)` gate depends on PROFs
        // granted by an earlier unsolved feat sees the right baseline.
        let mut baseline = Character::from_identity(original.identity.clone());
        let mut feat_states: Vec<FeatState> = Vec::new();
        for pending in &all_pending {
            let Some(feat_def) = fi.get(pending.name.as_str()) else {
                continue;
            };
            let Some(assign_defs) = feat_def.assign.as_ref() else {
                continue;
            };
            let stored = original.features.get_inputs(&pending.name);
            let stored_usable = stored_inputs_usable(feat_def, stored);

            // Index into the interactive-filtered position matches the
            // order `FeatureDefinition::assign_inner` uses to consume
            // stored inputs (feature.rs:393). Using the index (not a
            // `stored_iter.next()` that advances inside `filter_map`)
            // keeps alignment stable when `outer_group(expr)` returns
            // `None` — that skip must drop the stored slot too.
            let assigns: Vec<AssignData> = assign_defs
                .iter()
                .filter(|assignment| {
                    assignment.when == WhenCondition::OnFeatureAdd && assignment.is_interactive()
                })
                .enumerate()
                .filter_map(|(idx, assignment)| {
                    let mask = outer_group(&assignment.expr)?;
                    let arg_range = scan_arg_range(&assignment.expr)?;
                    let arg_count = assignment.expr.arg_slot_count(Attribute::arg_index);
                    let forced = stored_usable
                        .then(|| stored.get(idx).map(|input| input.args.clone()))
                        .flatten();
                    Some(AssignData {
                        expr: assignment.expr.clone(),
                        mask,
                        arg_range,
                        args: vec![0; arg_count],
                        forced,
                    })
                })
                .collect();
            if assigns.is_empty() {
                continue;
            }
            feat_states.push(FeatState {
                def: feat_def,
                pending,
                assigns,
            });
        }

        // Solve. Ignore the success flag — even on partial solve the
        // last-tried args in each assign are the best-effort prefill that
        // the modal will pick up.
        let _ = solve_all(&mut feat_states, &baseline, original);

        // Emit PendingInputs per feat, detecting replacement choices against
        // the pre-apply baseline (so `meets_prerequisites` sees the character
        // as it existed right before this feat). Then apply the feat to
        // advance baseline for the next iteration. Stored-input feats are
        // skipped for emit — their inputs live in `original`, modal won't
        // show them — but they still `apply` to move baseline forward.
        let mut inputs: Vec<PendingInputs> = Vec::new();
        for state in &feat_states {
            let inputs_vec: Vec<AssignInputs> = state
                .assigns
                .iter()
                .map(|assign| AssignInputs {
                    args: assign.args.clone(),
                    ..AssignInputs::default()
                })
                .collect();
            let was_forced = state.assigns.iter().all(|assign| assign.forced.is_some());

            let prefilled_replacement = (!was_forced)
                .then(|| {
                    detect_replacement(
                        state.pending,
                        state.def,
                        original,
                        fi,
                        &pending_keys,
                        &baseline,
                    )
                })
                .flatten();

            state.def.apply(
                state.pending.level,
                &mut baseline,
                WhenCondition::OnFeatureAdd,
                &inputs_vec,
            );

            if was_forced {
                continue;
            }
            if let Some(mut pi) = state.pending.pending_inputs(state.def, original) {
                pi.prefill = inputs_vec;
                pi.prefilled_replacement = prefilled_replacement;
                inputs.push(pi);
            }
        }
        inputs
    })
}

/// Find a replacement feature in `original.features` that the user chose for
/// `pending`. Returns the stored feature's name if:
///
/// 1. `pending` has a non-`None` `replace_with` filter.
/// 2. `original.features` does NOT already contain `(pending.name,
///    pending.source)` — if it does, F is present and wasn't replaced.
/// 3. An original feature X exists with `X.source == pending.source`, `(X.name,
///    X.source) ∉ pending_keys` (X isn't a separate slot the identity already
///    expects), `fi.get(X.name).replace_with_matches(F)`, and
///    `X_def.meets_prerequisites(baseline)`.
///
/// First match wins — `original.features` preserves insertion order, and a
/// single slot hosts exactly one replacement.
fn detect_replacement(
    pending: &PendingFeature,
    feat_def: &FeatureDefinition,
    original: &Character,
    fi: &BTreeMap<Box<str>, FeatureDefinition>,
    pending_keys: &BTreeSet<(&str, &FeatureSource)>,
    baseline: &Character,
) -> Option<String> {
    if matches!(feat_def.replace_with, ReplaceWith::None) {
        return None;
    }
    let already_present = original
        .features
        .iter()
        .any(|feature| feature.name == pending.name && feature.source == pending.source);
    if already_present {
        return None;
    }
    original
        .features
        .iter()
        .find(|candidate| {
            if candidate.source != pending.source {
                return false;
            }
            if pending_keys.contains(&(candidate.name.as_str(), &candidate.source)) {
                return false;
            }
            let Some(candidate_def) = fi.get(candidate.name.as_str()) else {
                return false;
            };
            feat_def.replace_with.matches(candidate_def)
                && candidate_def.meets_prerequisites(baseline)
        })
        .map(|candidate| candidate.name.clone())
}

/// Check whether stored `inputs` align with the feature's interactive
/// assigns — every `AssignInputs.args` must be sized to its corresponding
/// expression's `arg_slot_count`. Returns `false` for the empty-slice case
/// too, so callers can branch on a single predicate.
fn stored_inputs_usable(feat_def: &FeatureDefinition, stored: &[AssignInputs]) -> bool {
    if stored.is_empty() {
        return false;
    }
    let Some(assigns) = feat_def.assign.as_ref() else {
        return true;
    };
    let interactive_exprs = assigns.iter().filter(|assignment| {
        assignment.when == WhenCondition::OnFeatureAdd && assignment.is_interactive()
    });
    stored
        .iter()
        .zip(interactive_exprs)
        .all(|(input, assignment)| {
            input.args.len() == assignment.expr.arg_slot_count(Attribute::arg_index)
        })
}

/// If `original` has no feature in the `Generation` category (legacy
/// character pre-dating the generation-feature system), synthesize a
/// generation User(0) feature with `inputs` set to the "base" abilities
/// (original scores minus identity contributions), prepend it to
/// `clean.features`, and bump `clean.abilities` to match `original`. On
/// the next rebuild the synthesized feature is applied first and the same
/// identity contributions stack on top, landing at `original.abilities`.
///
/// Prefers `Generation: Fixed Preset` when the base scores form a permutation
/// of the standard 5e 2024 array `[15, 14, 13, 12, 10, 8]`; otherwise falls
/// back to `Generation: Custom`.
fn migrate_legacy_abilities(clean: &mut Character, original: &Character) {
    if original.features.has_category(FeatureCategory::Generation) {
        return;
    }

    let mut args: Vec<i32> = Vec::with_capacity(Ability::VARIANTS.len());
    for &ability in Ability::VARIANTS {
        let orig = original.ability_score(ability) as i32;
        let current = clean.ability_score(ability) as i32;
        args.push(orig - (current - 8));
        if orig != current {
            clean.set_ability(ability, orig as u32);
        }
    }

    let name = if is_fixed_preset(&args) {
        "Generation: Fixed Preset"
    } else {
        "Generation: Custom"
    };

    clean.features.list.insert(
        0,
        Feature {
            name: name.to_string(),
            label: None,
            description: String::new(),
            applied: true,
            category: FeatureCategory::Generation,
            source: FeatureSource::User(0),
            inputs: vec![AssignInputs {
                args,
                ..AssignInputs::default()
            }],
        },
    );
}

/// Match the standard 5e 2024 preset `[15, 14, 13, 12, 10, 8]` in any order.
fn is_fixed_preset(args: &[i32]) -> bool {
    let Ok(mut sorted): Result<[i32; 6], _> = args.try_into() else {
        return false;
    };
    sorted.sort_unstable();
    sorted == [8, 10, 12, 13, 14, 15]
}

/// Apply class levels in canonical order.
///
/// Algorithm: after classes[0] L1 (primary entry), check whether classes[0]
/// meets its own prereq against the current clean character. If it doesn't,
/// we skip multiclass logic entirely and just level classes[0] to target —
/// classes[1..] aren't applied. If it does, we loop: at each step, filter
/// classes[1..] by prereq against current clean; if any multiclass is
/// eligible (passes prereq AND has remaining target levels), take its next
/// level (preferring lower index); otherwise level classes[0]. Stops when
/// all reachable targets are met or progress halts.
fn apply_classes_interleaved(
    registry: &RulesRegistry,
    fi: &BTreeMap<Box<str>, FeatureDefinition>,
    clean: &mut Character,
    original: &Character,
    extra_inputs: &ApplyInputs,
) -> Result<(), RebuildError> {
    let class_cache = registry.classes().cache().read_untracked();
    let n_classes = clean.identity.classes.len();
    if n_classes == 0 {
        return Ok(());
    }
    let targets: Vec<u32> = clean
        .identity
        .classes
        .iter()
        .map(|class_level| class_level.level)
        .collect();
    if targets.iter().all(|&t| t == 0) {
        return Ok(());
    }

    let mut applied: Vec<u32> = vec![0; n_classes];
    let mut character_level: u32 = 0;

    // CL1: classes[0] at class level 1 — primary, no prereq check.
    if targets[0] > 0 && !clean.identity.classes[0].class.is_empty() {
        apply_class_level(fi, &class_cache, clean, original, 0, 1, extra_inputs)?;
        applied[0] = 1;
        character_level = 1;
        apply_user_features_at_level(original, clean, fi, character_level, extra_inputs)?;
    }

    // Hold the class-index lock for the whole loop: pick_next_class checks
    // prereqs on every iteration, acquiring it per step would thrash.
    registry.with_class_entries(|entries| -> Result<(), RebuildError> {
        while let Some(i) = pick_next_class(clean, &targets, &applied, entries) {
            let next_class_lvl = applied[i] + 1;
            apply_class_level(
                fi,
                &class_cache,
                clean,
                original,
                i,
                next_class_lvl,
                extra_inputs,
            )?;
            applied[i] = next_class_lvl;
            character_level += 1;
            apply_user_features_at_level(original, clean, fi, character_level, extra_inputs)?;
        }
        Ok(())
    })?;

    // After the loop exits: primary is maxed out, but if any multiclass still
    // has unapplied target levels, its prereq never passed during the build —
    // that multiclass is illegal.
    applied
        .iter()
        .zip(&targets)
        .enumerate()
        .skip(1)
        .find(|(i, (applied, target))| {
            applied < target && !clean.identity.classes[*i].class.is_empty()
        })
        .map_or(Ok(()), |(i, _)| {
            Err(RebuildError::MulticlassPrereq {
                class: clean.identity.classes[i].class.clone(),
            })
        })
}

/// Pick the next class to level up per the filter-and-apply algorithm.
/// `entries` is the locked class-index map from `with_class_entries`.
/// Returns `None` when no class has remaining levels, or when multiclass
/// logic is disabled and the primary is already at target.
///
/// Primary prereq is re-checked every call because earlier ASIs / background
/// boosts can flip the result as the build progresses.
fn pick_next_class(
    clean: &Character,
    targets: &[u32],
    applied: &[u32],
    entries: &BTreeMap<Box<str>, ClassIndexEntry>,
) -> Option<usize> {
    let meets_prereq = |class_name: &str| {
        entries
            .get(class_name)
            .is_none_or(|entry| entry.meets_prerequisites(clean))
    };

    let idx = if meets_prereq(clean.identity.classes[0].class.as_str()) {
        // Filter classes[1..] by prereq + remaining levels; take first match,
        // fall back to primary (0).
        clean
            .identity
            .classes
            .iter()
            .enumerate()
            .skip(1)
            .find(|(i, class_level)| {
                !class_level.class.is_empty()
                    && applied[*i] < targets[*i]
                    && meets_prereq(class_level.class.as_str())
            })
            .map_or(0, |(i, _)| i)
    } else {
        0
    };

    let cl = &clean.identity.classes[idx];
    (!cl.class.is_empty() && applied[idx] < targets[idx]).then_some(idx)
}

fn apply_class_level(
    fi: &BTreeMap<Box<str>, FeatureDefinition>,
    class_cache: &BTreeMap<Box<str>, ClassDefinition>,
    clean: &mut Character,
    original: &Character,
    class_idx: usize,
    class_level: u32,
    extra_inputs: &ApplyInputs,
) -> Result<(), RebuildError> {
    let class_name = clean.identity.classes[class_idx].class.clone();
    let class_def =
        class_cache
            .get(class_name.as_str())
            .ok_or_else(|| RebuildError::MissingDefinition {
                kind: "class",
                name: class_name.clone(),
            })?;
    clean.identity.classes[class_idx].hit_die_sides = class_def.hit_die;
    let pending: Vec<PendingFeature> =
        collect_class_features(clean, class_idx, class_level, class_def, fi).collect();
    apply_pending(fi, clean, &pending, original, extra_inputs)?;
    clean.applied.mark_level(&class_name, class_level);
    Ok(())
}

/// Apply a batch of pending features. Replacements from the modal are
/// resolved upfront so the correct feature (after any user-chosen swap)
/// drives input lookup. Each resolved pending is applied via
/// `apply_new_feature` with its own stored-or-modal inputs so stackable
/// features with the same name don't collide.
fn apply_pending(
    fi: &BTreeMap<Box<str>, FeatureDefinition>,
    clean: &mut Character,
    pending: &[PendingFeature],
    original: &Character,
    extra_inputs: &ApplyInputs,
) -> Result<(), RebuildError> {
    let resolved = resolve_replacements(pending, &extra_inputs.replacements, fi);
    for pending_feature in &resolved {
        let Some(feat_def) = fi.get(pending_feature.name.as_str()) else {
            return Err(RebuildError::MissingDefinition {
                kind: "feature",
                name: pending_feature.name.clone(),
            });
        };
        let inputs =
            inputs_for_pending(pending_feature, original, extra_inputs, feat_def.stackable);
        apply_new_feature(fi, clean, pending_feature, &inputs);
    }
    Ok(())
}

/// Resolve stored inputs for a single pending feature. Stackable features
/// require exact source match so multiple instances (e.g. ASI at Monk L4 and
/// L8) don't share storage. Non-stackable features match by name alone —
/// tolerates source encoding drift between versions (e.g. a subclass feature
/// stored with `Class(X, N)` on older saves when collect now generates
/// `Subclass(X, SC, N)`).
///
/// Falls back to `extra_inputs` (modal input) when nothing stored.
fn inputs_for_pending(
    pending_feature: &PendingFeature,
    original: &Character,
    extra_inputs: &ApplyInputs,
    stackable: bool,
) -> Vec<AssignInputs> {
    let stored: Vec<AssignInputs> = original
        .features
        .iter()
        .find(|feature| {
            feature.name == pending_feature.name
                && feature.applied
                && (!stackable || feature.source == pending_feature.source)
        })
        .map(|feature| feature.inputs.clone())
        .unwrap_or_default();

    if !stored.is_empty() {
        stored
    } else {
        let key = FeatureKey::from_pending(pending_feature);
        extra_inputs
            .feature_inputs
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }
}

fn apply_user_features_at_level(
    original: &Character,
    clean: &mut Character,
    fi: &BTreeMap<Box<str>, FeatureDefinition>,
    level: u32,
    extra_inputs: &ApplyInputs,
) -> Result<(), RebuildError> {
    let pending: Vec<PendingFeature> = original
        .features
        .iter()
        .filter(|feature| matches!(&feature.source, FeatureSource::User(l) if *l == level))
        .map(|feature| PendingFeature {
            name: feature.name.clone(),
            source: feature.source.clone(),
            level,
        })
        .collect();
    if pending.is_empty() {
        return Ok(());
    }
    apply_pending(fi, clean, &pending, original, extra_inputs)
}

/// Copy non-rebuildable user data from original into clean. Called after
/// `build_clean` finishes its apply pipeline but before the store update.
fn merge_preserved(clean: &mut Character, original: &Character) {
    clean.id = original.id;
    clean.shared = original.shared;
    clean.equipment = original.equipment.clone();
    clean.personality = original.personality.clone();
    clean.notes = original.notes.clone();
    clean.updated_at = original.updated_at;

    // In-game counters on CombatStats — preserve user state, keep recomputed
    // hp_max (hp_current clamps against the new max inside the method).
    clean.combat.merge_play_state(&original.combat);

    // XP: bump to the threshold of the resulting total level (matches
    // `apply_level` behavior so XP stays consistent with level).
    let xp_threshold = clean.xp_threshold();
    if clean.identity.experience_points < xp_threshold {
        clean.identity.experience_points = xp_threshold;
    }

    // Per-class hit dice used — match by class name (indices already aligned
    // since identity was cloned from original).
    for (clean_class, orig_class) in clean
        .identity
        .classes
        .iter_mut()
        .zip(&original.identity.classes)
    {
        if clean_class.class == orig_class.class {
            clean_class.hit_dice_used = orig_class.hit_dice_used;
        }
    }

    // Spell slot `used` counters — match pool+level, clamp to new total.
    for (pool, clean_slots) in clean.spell_slots.iter_mut() {
        let Some(orig_slots) = original.spell_slots.get(pool) else {
            continue;
        };
        for (i, slot) in clean_slots.iter_mut().enumerate() {
            if let Some(orig_slot) = orig_slots.get(i) {
                slot.used = orig_slot.used.min(slot.total);
            }
        }
    }

    // feature_data: preserve used counters on Points/Die fields and user's
    // Choice picks (Metamagic, etc.). Clean has the fresh field structure
    // from the current definition; we overlay per-field values from
    // original where applicable.
    for (name, clean_data) in clean.features.data_mut() {
        let Some(orig_data) = original.features.get(name) else {
            continue;
        };
        for clean_field in clean_data.fields.iter_mut() {
            let Some(orig_field) = orig_data
                .fields
                .iter()
                .find(|orig_field| orig_field.name == clean_field.name)
            else {
                continue;
            };
            match (&mut clean_field.value, &orig_field.value) {
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
                    // Fill empty (default) clean slots from original's picks.
                    // `zip` caps at the shorter length: if the definition now
                    // grants more slots than before, extra slots stay empty
                    // for the user to pick; if it grants fewer, trailing
                    // original picks are dropped.
                    for (clean_opt, orig_opt) in options.iter_mut().zip(orig_options) {
                        if clean_opt.name.is_empty() && !orig_opt.name.is_empty() {
                            *clean_opt = orig_opt.clone();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Restore user-selected spells from the original into clean.
    restore_all_spell_selections(original.features.data(), clean.features.data_mut());
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::model::{
        AssignInputs, ClassLevel, Die, Feature, FeatureData, FeatureField, FeatureValue,
    };

    fn feature(name: &str, source: FeatureSource) -> Feature {
        Feature {
            name: name.to_string(),
            source,
            applied: true,
            ..Feature::default()
        }
    }

    #[wasm_bindgen_test]
    fn merge_preserves_equipment_personality_notes() {
        let mut original = Character::default();
        original.notes = "important notes".into();
        original.personality.history = "backstory".into();

        let mut clean = Character::default();
        clean.notes = String::new();
        clean.personality.history = String::new();

        merge_preserved(&mut clean, &original);

        assert_eq!(clean.notes, "important notes");
        assert_eq!(clean.personality.history, "backstory");
    }

    #[wasm_bindgen_test]
    fn merge_preserves_hp_current_and_death_saves() {
        let mut original = Character::default();
        original.combat.hp_current = 7;
        original.combat.hp_temp = 3;
        original.combat.death_save_successes = 2;
        original.combat.death_save_failures = 1;

        let mut clean = Character::default();
        clean.combat.hp_max = 20;
        clean.combat.hp_current = 20;

        merge_preserved(&mut clean, &original);

        assert_eq!(clean.combat.hp_current, 7);
        assert_eq!(clean.combat.hp_temp, 3);
        assert_eq!(clean.combat.death_save_successes, 2);
        assert_eq!(clean.combat.death_save_failures, 1);
    }

    #[wasm_bindgen_test]
    fn merge_clamps_hp_current_to_new_max() {
        let mut original = Character::default();
        original.combat.hp_current = 100;

        let mut clean = Character::default();
        clean.combat.hp_max = 20;
        clean.combat.hp_current = 20;

        merge_preserved(&mut clean, &original);

        assert_eq!(clean.combat.hp_current, 20);
    }

    #[wasm_bindgen_test]
    fn merge_preserves_hit_dice_used_per_class() {
        let mut original = Character::default();
        original.identity.classes = vec![
            ClassLevel {
                class: "Monk".into(),
                level: 3,
                hit_dice_used: 2,
                ..ClassLevel::default()
            },
            ClassLevel {
                class: "Wizard".into(),
                level: 2,
                hit_dice_used: 1,
                ..ClassLevel::default()
            },
        ];

        let mut clean = Character::default();
        clean.identity.classes = vec![
            ClassLevel {
                class: "Monk".into(),
                level: 3,
                hit_dice_used: 0,
                ..ClassLevel::default()
            },
            ClassLevel {
                class: "Wizard".into(),
                level: 2,
                hit_dice_used: 0,
                ..ClassLevel::default()
            },
        ];

        merge_preserved(&mut clean, &original);

        assert_eq!(clean.identity.classes[0].hit_dice_used, 2);
        assert_eq!(clean.identity.classes[1].hit_dice_used, 1);
    }

    #[wasm_bindgen_test]
    fn merge_preserves_feature_data_used_counters() {
        let mut original = Character::default();
        let mut orig_data = FeatureData::default();
        orig_data.fields.push(FeatureField {
            name: "Ki Points".into(),
            label: None,
            description: String::new(),
            value: FeatureValue::Points { used: 2, max: 3 },
        });
        orig_data.fields.push(FeatureField {
            name: "Hit Dice".into(),
            label: None,
            description: String::new(),
            value: FeatureValue::Die {
                die: Die {
                    amount: 3,
                    sides: 8,
                },
                used: 1,
            },
        });
        original.features.insert("Martial Arts".into(), orig_data);

        let mut clean = Character::default();
        let mut clean_data = FeatureData::default();
        clean_data.fields.push(FeatureField {
            name: "Ki Points".into(),
            label: None,
            description: String::new(),
            value: FeatureValue::Points { used: 0, max: 5 },
        });
        clean_data.fields.push(FeatureField {
            name: "Hit Dice".into(),
            label: None,
            description: String::new(),
            value: FeatureValue::Die {
                die: Die {
                    amount: 5,
                    sides: 8,
                },
                used: 0,
            },
        });
        clean.features.insert("Martial Arts".into(), clean_data);

        merge_preserved(&mut clean, &original);

        let data = clean.features.get("Martial Arts").unwrap();
        let points = &data.fields[0].value;
        assert!(matches!(points, FeatureValue::Points { used: 2, max: 5 }));
        let die = &data.fields[1].value;
        assert!(matches!(
            die,
            FeatureValue::Die {
                used: 1,
                die: Die {
                    amount: 5,
                    sides: 8
                }
            }
        ));
    }

    #[wasm_bindgen_test]
    fn merge_clamps_used_counters_to_new_max() {
        let mut original = Character::default();
        let mut orig_data = FeatureData::default();
        orig_data.fields.push(FeatureField {
            name: "Ki Points".into(),
            label: None,
            description: String::new(),
            value: FeatureValue::Points { used: 10, max: 10 },
        });
        original.features.insert("Feat".into(), orig_data);

        let mut clean = Character::default();
        let mut clean_data = FeatureData::default();
        clean_data.fields.push(FeatureField {
            name: "Ki Points".into(),
            label: None,
            description: String::new(),
            value: FeatureValue::Points { used: 0, max: 3 },
        });
        clean.features.insert("Feat".into(), clean_data);

        merge_preserved(&mut clean, &original);

        let used = match &clean.features.get("Feat").unwrap().fields[0].value {
            FeatureValue::Points { used, .. } => *used,
            _ => panic!("expected Points"),
        };
        assert_eq!(used, 3);
    }

    #[wasm_bindgen_test]
    fn inputs_for_pending_matches_source_for_stackable() {
        let mut original = Character::default();
        let asi4_inputs = vec![AssignInputs {
            args: vec![0, 0, 0, 0, 2, 0],
            dice: Default::default(),
        }];
        let asi8_inputs = vec![AssignInputs {
            args: vec![0, 0, 0, 0, 0, 2],
            dice: Default::default(),
        }];
        original.features.list.push(Feature {
            inputs: asi4_inputs.clone(),
            ..feature(
                "Ability Score Improvement",
                FeatureSource::Class("Monk".into(), 4),
            )
        });
        original.features.list.push(Feature {
            inputs: asi8_inputs.clone(),
            ..feature(
                "Ability Score Improvement",
                FeatureSource::Class("Monk".into(), 8),
            )
        });

        let empty = ApplyInputs::default();

        let inputs_at_4 = inputs_for_pending(
            &PendingFeature {
                name: "Ability Score Improvement".into(),
                source: FeatureSource::Class("Monk".into(), 4),
                level: 4,
            },
            &original,
            &empty,
            true,
        );
        let inputs_at_8 = inputs_for_pending(
            &PendingFeature {
                name: "Ability Score Improvement".into(),
                source: FeatureSource::Class("Monk".into(), 8),
                level: 8,
            },
            &original,
            &empty,
            true,
        );

        assert_eq!(inputs_at_4, asi4_inputs);
        assert_eq!(inputs_at_8, asi8_inputs);
    }

    #[wasm_bindgen_test]
    fn inputs_for_pending_falls_back_to_extra_inputs_when_stored_empty() {
        let mut original = Character::default();
        original
            .features
            .list
            .push(feature("Mystery", FeatureSource::User(0)));

        let modal_inputs = vec![AssignInputs {
            args: vec![42],
            dice: Default::default(),
        }];
        let mut extra = ApplyInputs::default();
        extra.feature_inputs.insert(
            FeatureKey::new("Mystery", FeatureSource::User(0)),
            modal_inputs.clone(),
        );

        let inputs = inputs_for_pending(
            &PendingFeature {
                name: "Mystery".into(),
                source: FeatureSource::User(0),
                level: 0,
            },
            &original,
            &extra,
            false,
        );

        assert_eq!(inputs, modal_inputs);
    }

    #[wasm_bindgen_test]
    fn is_fixed_preset_accepts_standard_array_in_any_order() {
        assert!(is_fixed_preset(&[15, 14, 13, 12, 10, 8]));
        assert!(is_fixed_preset(&[8, 10, 12, 13, 14, 15]));
        assert!(is_fixed_preset(&[13, 15, 8, 14, 10, 12]));
    }

    #[wasm_bindgen_test]
    fn is_fixed_preset_rejects_mismatch() {
        assert!(!is_fixed_preset(&[15, 15, 13, 12, 10, 8])); // duplicate 15
        assert!(!is_fixed_preset(&[16, 14, 13, 12, 10, 8])); // out-of-set
        assert!(!is_fixed_preset(&[14, 14, 13, 12, 10, 9])); // 9 disallowed, sum off
        assert!(!is_fixed_preset(&[15, 14, 13, 12, 10])); // wrong length
        assert!(!is_fixed_preset(&[8, 8, 8, 8, 8, 8])); // all defaults
    }

    fn feat_def(
        name: &str,
        category: FeatureCategory,
        replace_with: ReplaceWith,
    ) -> FeatureDefinition {
        FeatureDefinition {
            name: name.to_string(),
            label: None,
            description: String::new(),
            stackable: false,
            category,
            replace_with,
            spells: None,
            fields: BTreeMap::new(),
            assign: None,
            prerequisites: None,
        }
    }

    #[wasm_bindgen_test]
    fn detect_replacement_finds_user_swap_in_slot() {
        let slot_source = FeatureSource::Class("Rogue".into(), 3);
        let slot_def = feat_def("Rogue Subclass", FeatureCategory::Class, ReplaceWith::Any);
        let swap_def = feat_def(
            "Arcane Trickster",
            FeatureCategory::General,
            ReplaceWith::None,
        );

        let mut fi: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        fi.insert(slot_def.name.clone().into(), slot_def.clone());
        fi.insert(swap_def.name.clone().into(), swap_def.clone());

        let mut original = Character::default();
        original
            .features
            .list
            .push(feature("Arcane Trickster", slot_source.clone()));

        let pending = PendingFeature {
            name: "Rogue Subclass".into(),
            source: slot_source.clone(),
            level: 3,
        };
        let pending_keys: BTreeSet<(&str, &FeatureSource)> =
            [(pending.name.as_str(), &pending.source)]
                .into_iter()
                .collect();
        let baseline = Character::default();

        let found = detect_replacement(
            &pending,
            &slot_def,
            &original,
            &fi,
            &pending_keys,
            &baseline,
        );
        assert_eq!(found, Some("Arcane Trickster".into()));
    }

    #[wasm_bindgen_test]
    fn detect_replacement_skips_when_slot_already_present() {
        let slot_source = FeatureSource::Class("Rogue".into(), 3);
        let slot_def = feat_def("Rogue Subclass", FeatureCategory::Class, ReplaceWith::Any);
        let fi: BTreeMap<Box<str>, FeatureDefinition> =
            std::iter::once((slot_def.name.clone().into(), slot_def.clone())).collect();

        let mut original = Character::default();
        // F itself is in original — user never swapped.
        original
            .features
            .list
            .push(feature("Rogue Subclass", slot_source.clone()));

        let pending = PendingFeature {
            name: "Rogue Subclass".into(),
            source: slot_source.clone(),
            level: 3,
        };
        let pending_keys: BTreeSet<(&str, &FeatureSource)> =
            [(pending.name.as_str(), &pending.source)]
                .into_iter()
                .collect();
        let baseline = Character::default();

        let found = detect_replacement(
            &pending,
            &slot_def,
            &original,
            &fi,
            &pending_keys,
            &baseline,
        );
        assert!(found.is_none());
    }

    #[wasm_bindgen_test]
    fn detect_replacement_returns_none_when_not_replaceable() {
        let slot_source = FeatureSource::Class("Rogue".into(), 3);
        let slot_def = feat_def("Cunning Action", FeatureCategory::Class, ReplaceWith::None);
        let fi: BTreeMap<Box<str>, FeatureDefinition> =
            std::iter::once((slot_def.name.clone().into(), slot_def.clone())).collect();

        let original = Character::default();
        let pending = PendingFeature {
            name: "Cunning Action".into(),
            source: slot_source.clone(),
            level: 3,
        };
        let pending_keys: BTreeSet<(&str, &FeatureSource)> =
            [(pending.name.as_str(), &pending.source)]
                .into_iter()
                .collect();
        let baseline = Character::default();

        assert!(
            detect_replacement(
                &pending,
                &slot_def,
                &original,
                &fi,
                &pending_keys,
                &baseline
            )
            .is_none()
        );
    }
}
