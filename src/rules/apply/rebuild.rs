use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::ReadUntracked;
use strum::VariantArray;

use crate::{
    model::{
        Ability, Applied, AssignInputs, Attribute, Character, Feature, FeatureCategory,
        FeatureSource,
    },
    rules::{
        ClassDefinition, ClassIndexEntry, DefinitionStore, RulesRegistry, WhenCondition,
        apply::{
            collect::{
                collect_background_features, collect_class_features, collect_pending_features,
                collect_species_features,
            },
            compute,
            pending::{ApplyInputs, FeatureKey, PendingFeature, PendingInputs},
            primitives::{apply_new_feature, resolve_replacements, restore_user_state},
            reconcile::reconcile_user_feature_sources,
            solver::{AssignData, FeatState, outer_group, scan_arg_range, solve_all},
        },
        feature::{FeatureDefinition, ReplaceWith},
        spells::SpellDefinition,
    },
};

const GENERATION_FIXED_PRESET: &str = "Generation: Fixed Preset";
const GENERATION_USER_DEFINED: &str = "Generation: User-Defined";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionKind {
    Class,
    Species,
    Background,
}

#[derive(Debug, Clone)]
pub enum RebuildError {
    MissingDefinition { kind: DefinitionKind, name: String },
    MulticlassPrereq { class: String },
}

/// Successful `build_clean` result: the rebuilt character plus per-feature
/// drift accounting. `skipped_features` lists User-source features whose
/// definition has gone missing (kept in `character.features` as orphans so the
/// user's pick / inputs / homebrew naming aren't lost). `removed_features`
/// lists identity-source features (Class / Subclass / Species / Background)
/// whose definition has gone missing — these are dropped from
/// `character.features` because the next rebuild will re-emit them under
/// whatever name the current data file uses. Empty-name User placeholders
/// (the "Add feature" slot before the user picks anything) are preserved
/// silently and counted in neither vec.
#[derive(Debug, Clone)]
pub struct RebuildOutcome {
    pub character: Character,
    pub skipped_features: Vec<String>,
    pub removed_features: Vec<String>,
}

/// Internal accumulator threaded through the apply pipeline. Converted to
/// `RebuildOutcome` at the end of `build_clean`.
#[derive(Default)]
struct RebuildAccum {
    skipped: Vec<String>,
    removed: Vec<String>,
}

/// Bundled apply-pipeline context. Cuts argument lists from 7-9 down to
/// `(ctx, clean, ...)` and locks the index/source/inputs/accum borrows
/// behind named fields (vs four adjacent `&BTreeMap`/`&Character` positionals
/// that were easy to swap by mistake).
struct RebuildCtx<'a> {
    feat_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
    spell_index: &'a BTreeMap<Box<str>, SpellDefinition>,
    original: &'a Character,
    extra_inputs: &'a ApplyInputs,
    accum: &'a mut RebuildAccum,
}

/// Output of `prepare_rebuild`: everything the rebuild caller needs to drive
/// silent-commit or the args modal.
pub struct RebuildPreview {
    /// Reconciled original character — feeds into `build_clean`.
    pub original: Character,
    /// Feats the modal cascade walks, in pipeline order. Starts at the first
    /// unsolved / rejected feat; `hidden=true` entries between editable
    /// ones are effective-stored feats the cascade applies silently.
    pub pending: Vec<PendingInputs>,
    /// Cascade seed: identity + every effective-stored feat applied in
    /// pipeline order up to (but not including) the first emitted feat.
    /// The modal layers `pending` on top of this, so `expr.analyze` on the
    /// first editable feat sees a correct baseline out of the gate.
    pub cascade_base: Character,
    /// `true` when pre-validation discarded non-empty stored inputs as
    /// ineffective — forces the modal to open even if `build_clean` would
    /// silent-match.
    pub had_rejections: bool,
}

/// Snapshot-level step 1: reconcile User-sourced features against identity
/// slots, then collect pending inputs that need user interaction.
#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "rebuild.prepare", skip_all)
)]
pub fn prepare_rebuild(mut original: Character, registry: &RulesRegistry) -> RebuildPreview {
    reconcile_user_feature_sources(&mut original, registry);
    let (pending, had_rejections, cascade_base) =
        collect_rebuild_pending_inputs(&original, registry);
    RebuildPreview {
        original,
        pending,
        cascade_base,
        had_rejections,
    }
}

/// Snapshot-level step 2: build a fresh `Character` from `default()`,
/// applying identity + User features in canonical order (User(0) → Species →
/// Background → classes round-robin with multiclass prereq gates), then
/// merging preserved user state (HP, used counters, spell selections,
/// equipment, personality, notes). Fails if a class / species / background
/// definition is missing from the registry caches or if a multiclass prereq
/// never passes during the build.
#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "rebuild.build_clean", skip_all)
)]
pub fn build_clean(
    original: &Character,
    registry: &RulesRegistry,
    extra_inputs: &ApplyInputs,
) -> Result<RebuildOutcome, RebuildError> {
    let mut clean = Character::from_identity(original.identity.clone());
    let mut accum = RebuildAccum::default();

    registry.with_apply_indexes(|feat_index, spell_index| -> Result<(), RebuildError> {
        let mut ctx = RebuildCtx {
            feat_index,
            spell_index,
            original,
            extra_inputs,
            accum: &mut accum,
        };

        // 1. User(0) features (e.g. Generation: * setting base abilities)
        apply_user_features_at_level(&mut ctx, &mut clean, 0);

        // 2. Species
        if !clean.identity.species.is_empty() {
            let species_cache = registry.species().cache().read_untracked();
            let species_def = species_cache
                .get(clean.identity.species.as_str())
                .ok_or_else(|| RebuildError::MissingDefinition {
                    kind: DefinitionKind::Species,
                    name: clean.identity.species.clone(),
                })?;
            let pending: Vec<PendingFeature> =
                collect_species_features(&clean, species_def, ctx.feat_index).collect();
            apply_pending(&mut ctx, &mut clean, &pending);
            clean.applied.species = true;
        }

        // 3. Background
        if !clean.identity.background.is_empty() {
            let bg_cache = registry.backgrounds().cache().read_untracked();
            let bg_def = bg_cache
                .get(clean.identity.background.as_str())
                .ok_or_else(|| RebuildError::MissingDefinition {
                    kind: DefinitionKind::Background,
                    name: clean.identity.background.clone(),
                })?;
            let pending: Vec<PendingFeature> =
                collect_background_features(&clean, bg_def, ctx.feat_index).collect();
            apply_pending(&mut ctx, &mut clean, &pending);
            clean.applied.background = true;
        }

        // 4. Classes — interleave class levels with per-step prereq filter for
        //    multiclasses (see `apply_classes_interleaved` for details).
        // Hold both class_index and class_entries for the whole pipeline:
        // pick_next_class checks prereqs on every iteration, re-acquiring
        // would thrash the locks.
        let class_index = registry.classes().cache().read_untracked();
        registry.with_class_entries(|class_entries| {
            apply_classes_interleaved(&mut ctx, &class_index, class_entries, &mut clean)
        })?;

        // 5. Legacy migration: characters built before the Generation-feature system
        //    have abilities edited directly. Convert those custom scores into a
        //    synthesized `Generation: User-Defined` feature so future rebuilds keep
        //    them intact.
        migrate_legacy_abilities(&mut clean, original);

        // compute creates the empty SPELL.READY/KNOWN slots that
        // restore_all_spell_selections fills from the original.
        compute(&mut clean, ctx.feat_index, ctx.spell_index);

        Ok(())
    })?;

    merge_preserved(&mut clean, original);
    Ok(RebuildOutcome {
        character: clean,
        skipped_features: accum.skipped,
        removed_features: accum.removed,
    })
}

/// Collect features scheduled for the rebuild that still need user input
/// (OnFeatureAdd interactive exprs and no stored inputs in the original).
/// Covers both User features already in `original.features` and identity
/// features (species/background/classes) that will be added during rebuild.
fn collect_rebuild_pending_inputs(
    original: &Character,
    registry: &RulesRegistry,
) -> (Vec<PendingInputs>, bool, Character) {
    // Returns (pending, had_rejections, cascade_base).
    registry.with_apply_indexes(|feat_index, spell_index| {
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
        let identity_pending = collect_pending_features(&snapshot, registry, feat_index);

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

        // Pipeline-order walk: dry-run each feat against `validation_baseline`
        // to check whether stored inputs still produce a derived-state change.
        // Stored that no-ops (e.g. Expertise on now-non-proficient skills,
        // half-migrated empty-arg entries) gets rejected → solver re-enumerates;
        // effective stored advances the baseline so downstream dry-runs see a
        // realistic state. `cascade_base` is captured just before the first
        // interactive feat so the modal opens on a correct pre-edit snapshot.
        //
        // TODO(perf): `stored_inputs_effective` clones the whole baseline
        // via `clone_lean` for its dry-run diff; the clone grows every
        // iteration → O(N² × feature_count) on L20. Same in
        // `solver::candidate_changes_baseline`. Proper fix needs a
        // derived-only clone, deferred to avoid breaking a future
        // spells-through-assign refactor (may write to features.data).
        let mut validation_baseline = Character::from_identity(original.identity.clone());
        let mut cascade_base: Option<Character> = None;
        let mut feat_states: Vec<FeatState> = Vec::new();
        let mut state_idx_by_pending: Vec<Option<usize>> = Vec::with_capacity(all_pending.len());
        let mut had_rejections = false;
        for pending in &all_pending {
            let Some(feat_def) = feat_index.get(pending.name.as_str()) else {
                state_idx_by_pending.push(None);
                continue;
            };
            let Some(assign_defs) = feat_def.assign.as_ref() else {
                // Pure-replaceable feats (e.g. Versatile: replace_with=Origin,
                // no assign) also emit in the modal so the user can pick a
                // replacement. Capture cascade_base here if no interactive
                // feat has done so yet — nothing before advanced
                // validation_baseline for this branch, so the base matches.
                if !matches!(feat_def.replace_with, ReplaceWith::None) && cascade_base.is_none() {
                    cascade_base = Some(validation_baseline.clone_lean());
                }
                state_idx_by_pending.push(None);
                continue;
            };
            let has_interactive_onadd = assign_defs
                .iter()
                .any(|asn| asn.when == WhenCondition::OnFeatureAdd && asn.is_interactive());
            if has_interactive_onadd && cascade_base.is_none() {
                cascade_base = Some(validation_baseline.clone_lean());
            }
            let stored = original.features.get_inputs(&pending.name, &pending.source);
            let effective = !has_interactive_onadd
                || stored_inputs_effective(
                    feat_def,
                    spell_index,
                    pending.level,
                    &validation_baseline,
                    stored,
                );
            if has_interactive_onadd && !effective && !stored.is_empty() {
                // Corrupt stored (e.g. Expertise on now non-proficient skills).
                // Silent-commit would replay it; caller opens the modal instead.
                had_rejections = true;
            }

            if has_interactive_onadd {
                // Idx = position in the interactive-filtered stream — the
                // same order `FeatureDefinition::assign_inner` uses to
                // consume stored inputs (feature.rs:393). Stable when
                // `outer_group` returns `None` (that filter_map skip drops
                // the stored slot with it).
                let assigns: Vec<AssignData> = assign_defs
                    .iter()
                    .filter(|asn| asn.when == WhenCondition::OnFeatureAdd && asn.is_interactive())
                    .enumerate()
                    .filter_map(|(idx, assignment)| {
                        let mask = outer_group(&assignment.expr)?;
                        let arg_range = scan_arg_range(&assignment.expr)?;
                        let arg_count = assignment.expr.arg_slot_count(Attribute::arg_index);
                        let forced = effective
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
                if !assigns.is_empty() {
                    state_idx_by_pending.push(Some(feat_states.len()));
                    feat_states.push(FeatState {
                        def: feat_def,
                        pending,
                        assigns,
                    });
                } else {
                    state_idx_by_pending.push(None);
                }
            } else {
                state_idx_by_pending.push(None);
            }

            if effective {
                feat_def.apply(
                    pending.level,
                    &mut validation_baseline,
                    WhenCondition::OnFeatureAdd,
                    stored,
                    spell_index,
                );
            }
        }
        // No interactive feats → caller short-circuits via `pending.is_empty()`;
        // fallback seed is the fully-advanced validation_baseline.
        let cascade_base = cascade_base.unwrap_or_else(|| validation_baseline.clone_lean());

        // Solver starts from identity (it re-applies every feat through
        // its own pipeline walk). Ignore the success flag — even a partial
        // solve leaves the best-effort prefill in each assign's `args`.
        let solver_baseline = Character::from_identity(original.identity.clone());
        let _ = solve_all(&mut feat_states, &solver_baseline, original);

        // Emit: walk all_pending. Interactive feats emit editable from
        // their solver-solved args; non-interactive with `assign` ride as
        // hidden once the visible section has opened. `emit_baseline`
        // advances through every applied feat so `detect_replacement`
        // sees the pre-apply state for each.
        let mut emit_baseline = Character::from_identity(original.identity.clone());
        let mut inputs: Vec<PendingInputs> = Vec::new();
        let mut emit_started = false;
        for (pending_idx, pending) in all_pending.iter().enumerate() {
            let Some(feat_def) = feat_index.get(pending.name.as_str()) else {
                continue;
            };
            if let Some(state_idx) = state_idx_by_pending[pending_idx] {
                let state = &feat_states[state_idx];
                let inputs_vec: Vec<AssignInputs> = state
                    .assigns
                    .iter()
                    .map(|assign| AssignInputs {
                        args: assign.args.clone(),
                        ..AssignInputs::default()
                    })
                    .collect();

                let prefilled_replacement = detect_replacement(
                    state.pending,
                    state.def,
                    original,
                    feat_index,
                    &pending_keys,
                    &emit_baseline,
                );

                state.def.apply(
                    state.pending.level,
                    &mut emit_baseline,
                    WhenCondition::OnFeatureAdd,
                    &inputs_vec,
                    spell_index,
                );

                if let Some(mut pi) = state.pending.pending_inputs(state.def, original) {
                    pi.prefill = inputs_vec;
                    pi.prefilled_replacement = prefilled_replacement;
                    inputs.push(pi);
                }
                emit_started = true;
            } else if feat_def.assign.is_some() {
                // apply_new_feature (not bare feat_def.apply) so the row
                // lands in features.list — caster_info reads it.
                apply_new_feature(feat_index, spell_index, &mut emit_baseline, pending, &[]);
                if emit_started {
                    inputs.push(PendingInputs::hidden_for_cascade(
                        pending.name.clone(),
                        feat_def,
                        pending.source.clone(),
                    ));
                }
            } else if !matches!(feat_def.replace_with, ReplaceWith::None) {
                // Pure-replaceable feat (no assigns to run, no interactive
                // ARGs). Emit so the modal can ask the user for a
                // replacement pick, pre-filled from `detect_replacement` if
                // original has a sibling at this slot.
                let prefilled_replacement = detect_replacement(
                    pending,
                    feat_def,
                    original,
                    feat_index,
                    &pending_keys,
                    &emit_baseline,
                );
                if let Some(mut pi) = pending.pending_inputs(feat_def, original) {
                    pi.prefilled_replacement = prefilled_replacement;
                    inputs.push(pi);
                }
                emit_started = true;
            }
        }
        (inputs, had_rejections, cascade_base)
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
///    expects), `feat_index.get(X.name).replace_with_matches(F)`, and
///    `X_def.meets_prerequisites(baseline)`.
///
/// First match wins — `original.features` preserves insertion order, and a
/// single slot hosts exactly one replacement.
fn detect_replacement(
    pending: &PendingFeature,
    feat_def: &FeatureDefinition,
    original: &Character,
    feat_index: &BTreeMap<Box<str>, FeatureDefinition>,
    pending_keys: &BTreeSet<(&str, &FeatureSource)>,
    baseline: &Character,
) -> Option<String> {
    if matches!(feat_def.replace_with, ReplaceWith::None) {
        return None;
    }
    // Single pass: F itself present in the slot → no replacement; else
    // first candidate whose def matches the filter + prerequisites wins.
    let mut candidate_name: Option<String> = None;
    for feature in original.features.iter() {
        if feature.source != pending.source {
            continue;
        }
        if feature.name == pending.name {
            return None;
        }
        if candidate_name.is_some() {
            continue;
        }
        if pending_keys.contains(&(feature.name.as_str(), &feature.source)) {
            continue;
        }
        let Some(candidate_def) = feat_index.get(feature.name.as_str()) else {
            continue;
        };
        if feat_def.replace_with.matches(candidate_def)
            && candidate_def.meets_prerequisites(baseline)
        {
            candidate_name = Some(feature.name.clone());
        }
    }
    candidate_name
}

/// Check whether a dry-run of `feat_def.apply(stored)` on `baseline` would
/// change any derived state. Rejects stored inputs that pass
/// `stored_inputs_usable` by shape but apply to slots no longer valid — e.g.
/// Expertise stored on skills the character has since dropped proficiency in,
/// so `if(@==1, @ += @ARG)` silently no-ops. Such stored inputs are "corrupted"
/// from the solver's viewpoint: their apply produces no observable change, so
/// forcing them freezes an invalid pick. Returning `false` downgrades them to
/// unsolved and lets the solver re-enumerate candidates.
fn stored_inputs_effective(
    feat_def: &FeatureDefinition,
    spell_index: &BTreeMap<Box<str>, SpellDefinition>,
    level: u32,
    baseline: &Character,
    stored: &[AssignInputs],
) -> bool {
    if !stored_inputs_usable(feat_def, stored) {
        return false;
    }
    let mut trial = baseline.clone_lean();
    feat_def.apply(
        level,
        &mut trial,
        WhenCondition::OnFeatureAdd,
        stored,
        spell_index,
    );
    !baseline.eq_derived(&trial)
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
/// back to `Generation: User-Defined`.
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
        GENERATION_FIXED_PRESET
    } else {
        GENERATION_USER_DEFINED
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
    ctx: &mut RebuildCtx<'_>,
    class_index: &BTreeMap<Box<str>, ClassDefinition>,
    class_entries: &BTreeMap<Box<str>, ClassIndexEntry>,
    clean: &mut Character,
) -> Result<(), RebuildError> {
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
        apply_class_level(ctx, class_index, clean, 0, 1)?;
        applied[0] = 1;
        character_level = 1;
        apply_user_features_at_level(ctx, clean, character_level);
    }

    while let Some(i) = pick_next_class(clean, &targets, &applied, class_entries) {
        let next_class_lvl = applied[i] + 1;
        apply_class_level(ctx, class_index, clean, i, next_class_lvl)?;
        applied[i] = next_class_lvl;
        character_level += 1;
        apply_user_features_at_level(ctx, clean, character_level);
    }

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
    ctx: &mut RebuildCtx<'_>,
    class_index: &BTreeMap<Box<str>, ClassDefinition>,
    clean: &mut Character,
    class_idx: usize,
    class_level: u32,
) -> Result<(), RebuildError> {
    let class_def = {
        let class_name = clean.identity.classes[class_idx].class.as_str();
        class_index
            .get(class_name)
            .ok_or_else(|| RebuildError::MissingDefinition {
                kind: DefinitionKind::Class,
                name: class_name.to_owned(),
            })?
    };
    clean.identity.classes[class_idx].hit_die_sides = class_def.hit_die;
    let pending: Vec<PendingFeature> =
        collect_class_features(clean, class_idx, class_level, class_def, ctx.feat_index).collect();
    apply_pending(ctx, clean, &pending);
    clean
        .applied
        .mark_level(&clean.identity.classes[class_idx].class, class_level);
    Ok(())
}

/// Apply a batch of pending features. Replacements from the modal are
/// resolved upfront so the correct feature (after any user-chosen swap)
/// drives input lookup. Each resolved pending is applied via
/// `apply_new_feature` with its own stored-or-modal inputs so stackable
/// features with the same name don't collide.
///
/// Pending features whose definition is missing from `feat_index` are routed by
/// source: User-source ones are forwarded via `accum` for the caller (which
/// preserves them in `clean.features` directly — see
/// `apply_user_features_at_level`); identity-source ones (Class / Subclass /
/// Species / Background) are dropped from the rebuild and recorded in
/// `accum.removed`. Either way the rebuild keeps going — no `Err` path for
/// missing per-feature definitions.
fn apply_pending(ctx: &mut RebuildCtx<'_>, clean: &mut Character, pending: &[PendingFeature]) {
    let resolved = resolve_replacements(pending, &ctx.extra_inputs.replacements, ctx.feat_index);
    for pending_feature in &resolved {
        if let Some(feat_def) = ctx.feat_index.get(pending_feature.name.as_str()) {
            let inputs = inputs_for_pending(
                pending_feature,
                ctx.original,
                ctx.extra_inputs,
                feat_def.stackable,
            );
            apply_new_feature(
                ctx.feat_index,
                ctx.spell_index,
                clean,
                pending_feature,
                &inputs,
            );
        } else if pending_feature.source.is_user() {
            // User-source missing-def features are normally pre-handled by
            // `apply_user_features_at_level`. Reaching here means a different
            // path fed a User pending in — log loudly so we notice.
            log::warn!(
                "apply_pending: unexpected User pending without pre-handle: {pending_feature:?}"
            );
            ctx.accum.skipped.push(pending_feature.name.clone());
        } else {
            log::warn!("rebuild: dropping obsolete identity feature {pending_feature:?}");
            ctx.accum.removed.push(pending_feature.name.clone());
        }
    }
}

/// Resolve inputs for a single pending feature. Modal-supplied
/// `extra_inputs` win over `original`'s stored inputs — the modal
/// pre-fills its forms from stored (or solver-solved args) and lets the
/// user override, so whatever comes back from submit is the authoritative
/// choice. Stackable features require exact source match so multiple
/// instances (e.g. ASI at Monk L4 and Monk L8) don't share storage;
/// non-stackable features match by name alone — tolerates source encoding
/// drift between versions (e.g. a subclass feature stored with
/// `Class(X, N)` on older saves when collect now generates
/// `Subclass(X, SC, N)`).
///
/// Returns empty when neither source has inputs.
fn inputs_for_pending(
    pending_feature: &PendingFeature,
    original: &Character,
    extra_inputs: &ApplyInputs,
    stackable: bool,
) -> Vec<AssignInputs> {
    let key = FeatureKey::from_pending(pending_feature);
    if let Some(inputs) = extra_inputs.feature_inputs.get(&key)
        && !inputs.is_empty()
    {
        return inputs.clone();
    }
    original
        .features
        .iter()
        .find(|feature| {
            feature.name == pending_feature.name
                && feature.applied
                && (!stackable || feature.source == pending_feature.source)
        })
        .map(|feature| feature.inputs.clone())
        .unwrap_or_default()
}

fn apply_user_features_at_level(ctx: &mut RebuildCtx<'_>, clean: &mut Character, level: u32) {
    // Two passes over User(level) features in `original`:
    //   1. Missing-def → preserve the original `Feature` directly in
    //      `clean.features.list` so the user's pick / inputs / homebrew name
    //      survive a rebuild. Empty-name placeholders ("Add feature" slot before
    //      the user picked anything) preserve silently; named-unknown ones (renamed
    //      feats, custom homebrew) are logged + counted as skipped. Direct
    //      iteration handles duplicate empty/identical-name slots correctly, which
    //      a `find`-based path through `apply_pending` would not.
    //   2. Has-def → flow through `apply_pending` normally.
    let mut pending: Vec<PendingFeature> = Vec::new();
    for feature in ctx.original.features.iter() {
        if !matches!(&feature.source, FeatureSource::User(l) if *l == level) {
            continue;
        }
        if ctx.feat_index.contains_key(feature.name.as_str()) {
            pending.push(PendingFeature {
                name: feature.name.clone(),
                source: feature.source.clone(),
                level,
            });
        } else {
            if !feature.name.is_empty() {
                log::warn!("rebuild: preserving user feature with no definition: {feature:?}");
                ctx.accum.skipped.push(feature.name.clone());
            }
            clean.features.list.push(feature.clone());
        }
    }
    if !pending.is_empty() {
        apply_pending(ctx, clean, &pending);
    }
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

    restore_user_state(original.features.data(), clean.features.data_mut());
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::{
        model::{
            AssignInputs, ClassLevel, Die, Feature, FeatureData, FeatureField, FeatureValue, Note,
            ProficiencyLevel, Skill,
        },
        rules::spells::EMPTY_SPELL_INDEX,
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
        original.notes = vec![Note {
            created_at: 42,
            level: 3,
            text: "important notes".into(),
        }];
        original.personality.history = "backstory".into();

        let mut clean = Character::default();
        clean.notes = Vec::new();
        clean.personality.history = String::new();

        merge_preserved(&mut clean, &original);

        assert_eq!(clean.notes.len(), 1);
        assert_eq!(clean.notes[0].text, "important notes");
        assert_eq!(clean.notes[0].level, 3);
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
    fn inputs_for_pending_prefers_extra_over_stored() {
        // Modal-submitted inputs override `original`'s stored inputs so
        // corrupted stored (e.g. Expertise on skills no longer proficient)
        // can be overwritten by the user's fresh pick in the modal.
        let stored_inputs = vec![AssignInputs {
            args: vec![99],
            ..AssignInputs::default()
        }];
        let mut stored_feature = feature("Mystery", FeatureSource::User(0));
        stored_feature.inputs = stored_inputs;
        let mut original = Character::default();
        original.features.list.push(stored_feature);

        let modal_inputs = vec![AssignInputs {
            args: vec![42],
            ..AssignInputs::default()
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
            name: name.into(),
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

        let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index.insert(slot_def.name.clone().into(), slot_def.clone());
        feat_index.insert(swap_def.name.clone().into(), swap_def.clone());

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
            &feat_index,
            &pending_keys,
            &baseline,
        );
        assert_eq!(found, Some("Arcane Trickster".into()));
    }

    #[wasm_bindgen_test]
    fn detect_replacement_skips_when_slot_already_present() {
        let slot_source = FeatureSource::Class("Rogue".into(), 3);
        let slot_def = feat_def("Rogue Subclass", FeatureCategory::Class, ReplaceWith::Any);
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> =
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
            &feat_index,
            &pending_keys,
            &baseline,
        );
        assert!(found.is_none());
    }

    #[wasm_bindgen_test]
    fn stored_inputs_effective_rejects_noop_expertise_on_non_proficient_skill() {
        let expertise_def: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Expertise",
            "stackable": true,
            "assign": [{
                "when": "OnFeatureAdd",
                "expr": "with(@SKILL._.PROF, guard(fold(and, @, in(@ARG, 0, 1)) and \
                         fold(+, @, @ARG) == 2, each(@, if(@ == 1, @ += @ARG))))",
            }],
        }))
        .unwrap();
        // Expertise args[5]=1 (History), args[14]=1 (Religion). Baseline has
        // neither Proficient → apply body `if(@==1,…)` no-ops for both →
        // derived state unchanged → effective == false.
        let stored = [AssignInputs {
            args: vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
            ..AssignInputs::default()
        }];
        let baseline = Character::default();
        assert!(!stored_inputs_effective(
            &expertise_def,
            &EMPTY_SPELL_INDEX,
            1,
            &baseline,
            &stored
        ));

        // Same stored, but now History is Proficient — apply bumps it to
        // Expertise (value 2) → skills map mutated → effective == true.
        let mut baseline_prof = Character::default();
        baseline_prof
            .skills
            .set(Skill::History, ProficiencyLevel::Proficient);
        baseline_prof
            .skills
            .set(Skill::Religion, ProficiencyLevel::Proficient);
        assert!(stored_inputs_effective(
            &expertise_def,
            &EMPTY_SPELL_INDEX,
            1,
            &baseline_prof,
            &stored
        ));
    }

    #[wasm_bindgen_test]
    fn apply_user_features_preserves_empty_name_silently() {
        // Empty-name User slots come from the "Add feature" button in the
        // build tab — the user clicked it but hasn't picked anything yet.
        // Rebuild must keep them around (otherwise the user's slot disappears)
        // and must not surface them as "skipped" — the user already knows.
        let mut original = Character::default();
        original.features.list.push(Feature {
            name: String::new(),
            source: FeatureSource::User(0),
            applied: false,
            ..Feature::default()
        });
        original.features.list.push(Feature {
            name: String::new(),
            source: FeatureSource::User(0),
            applied: false,
            ..Feature::default()
        });
        let mut clean = Character::from_identity(original.identity.clone());
        let mut accum = RebuildAccum::default();
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        let extra = ApplyInputs::default();

        let mut ctx = RebuildCtx {
            feat_index: &feat_index,
            spell_index: &EMPTY_SPELL_INDEX,
            original: &original,
            extra_inputs: &extra,
            accum: &mut accum,
        };
        apply_user_features_at_level(&mut ctx, &mut clean, 0);

        let empty_user_count = clean
            .features
            .list
            .iter()
            .filter(|feature| {
                feature.name.is_empty() && matches!(feature.source, FeatureSource::User(0))
            })
            .count();
        assert_eq!(empty_user_count, 2, "both empty placeholders preserved");
        assert!(
            accum.skipped.is_empty(),
            "empty-name doesn't count as skipped"
        );
        assert!(accum.removed.is_empty());
    }

    #[wasm_bindgen_test]
    fn apply_user_features_preserves_named_unknown_and_counts_skipped() {
        // Named-unknown User feature: either renamed in features.json (e.g.
        // "Generation: Custom" → "Generation: User-Defined") or homebrew the
        // user typed in. Either way preserve the entry — the user's inputs /
        // homebrew name shouldn't disappear silently — and surface a count
        // via accum.skipped so the post-rebuild toast can warn the user.
        let inputs = vec![AssignInputs {
            args: vec![15, 14, 13, 12, 10, 8],
            ..AssignInputs::default()
        }];
        let mut original = Character::default();
        original.features.list.push(Feature {
            name: "Generation: Custom".into(),
            source: FeatureSource::User(0),
            applied: true,
            inputs: inputs.clone(),
            ..Feature::default()
        });
        let mut clean = Character::from_identity(original.identity.clone());
        let mut accum = RebuildAccum::default();
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        let extra = ApplyInputs::default();

        let mut ctx = RebuildCtx {
            feat_index: &feat_index,
            spell_index: &EMPTY_SPELL_INDEX,
            original: &original,
            extra_inputs: &extra,
            accum: &mut accum,
        };
        apply_user_features_at_level(&mut ctx, &mut clean, 0);

        let preserved = clean
            .features
            .list
            .iter()
            .find(|feature| feature.name == "Generation: Custom")
            .expect("preserved in clean");
        assert_eq!(preserved.inputs, inputs, "user inputs survive");
        assert_eq!(accum.skipped, vec!["Generation: Custom".to_string()]);
        assert!(accum.removed.is_empty());
    }

    #[wasm_bindgen_test]
    fn apply_pending_drops_unknown_identity_feature_and_counts_removed() {
        // Identity-source (Class/Species/Background) features whose definition
        // is gone are obsolete — the next class re-emit will produce the new
        // name, so the old entry just lives forever as dead weight if we
        // preserve it. Drop and count.
        let mut clean = Character::default();
        let original = Character::default();
        let mut accum = RebuildAccum::default();
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        let extra = ApplyInputs::default();
        let pending = vec![PendingFeature {
            name: "Old Class Feat".into(),
            source: FeatureSource::Class("Wizard".into(), 1),
            level: 1,
        }];

        let mut ctx = RebuildCtx {
            feat_index: &feat_index,
            spell_index: &EMPTY_SPELL_INDEX,
            original: &original,
            extra_inputs: &extra,
            accum: &mut accum,
        };
        apply_pending(&mut ctx, &mut clean, &pending);

        assert!(
            !clean
                .features
                .list
                .iter()
                .any(|feature| feature.name == "Old Class Feat"),
            "obsolete identity feature dropped from clean"
        );
        assert_eq!(accum.removed, vec!["Old Class Feat".to_string()]);
        assert!(accum.skipped.is_empty());
    }

    #[wasm_bindgen_test]
    fn apply_user_features_routes_known_via_apply_pending() {
        // Mixed bag: empty placeholder (silent), homebrew (skipped), and a
        // known feature that has a definition (applied normally). Each lands
        // in the right bucket.
        let known_def = feat_def("Known Feat", FeatureCategory::General, ReplaceWith::None);
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> =
            std::iter::once((known_def.name.clone(), known_def.clone())).collect();

        let mut original = Character::default();
        original.features.list.push(Feature {
            name: String::new(),
            source: FeatureSource::User(0),
            applied: false,
            ..Feature::default()
        });
        original.features.list.push(Feature {
            name: "Homebrew Smite".into(),
            source: FeatureSource::User(0),
            applied: true,
            ..Feature::default()
        });
        original.features.list.push(Feature {
            name: "Known Feat".into(),
            source: FeatureSource::User(0),
            applied: true,
            ..Feature::default()
        });
        let mut clean = Character::from_identity(original.identity.clone());
        let mut accum = RebuildAccum::default();
        let extra = ApplyInputs::default();

        let mut ctx = RebuildCtx {
            feat_index: &feat_index,
            spell_index: &EMPTY_SPELL_INDEX,
            original: &original,
            extra_inputs: &extra,
            accum: &mut accum,
        };
        apply_user_features_at_level(&mut ctx, &mut clean, 0);

        assert_eq!(accum.skipped, vec!["Homebrew Smite".to_string()]);
        assert!(accum.removed.is_empty());
        assert!(
            clean
                .features
                .list
                .iter()
                .any(|feature| feature.name.is_empty())
        );
        assert!(
            clean
                .features
                .list
                .iter()
                .any(|feature| feature.name == "Homebrew Smite")
        );
        assert!(
            clean
                .features
                .list
                .iter()
                .any(|feature| feature.name == "Known Feat" && feature.applied)
        );
    }

    #[wasm_bindgen_test]
    fn detect_replacement_returns_none_when_not_replaceable() {
        let slot_source = FeatureSource::Class("Rogue".into(), 3);
        let slot_def = feat_def("Cunning Action", FeatureCategory::Class, ReplaceWith::None);
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> =
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
                &feat_index,
                &pending_keys,
                &baseline
            )
            .is_none()
        );
    }
}
