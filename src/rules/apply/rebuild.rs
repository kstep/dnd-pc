use std::collections::{BTreeMap, BTreeSet};

use strum::VariantArray;

use crate::{
    model::{
        Ability, AssignInputs, Attribute, Character, CharacterCore, Feature, FeatureCategory,
        FeatureSource,
    },
    rules::{
        FeaturesView, RecomputePending, RulesRegistry, WhenCondition,
        apply::{
            DefinitionCaches, compute, dry_run_apply_feature,
            pending::{ApplyInputs, FeatureKey, PendingFeature, PendingInputs},
            plan::level_up_plan,
            primitives::{apply_pending, cascade, detect_replacement, restore_user_state},
            reconcile::reconcile_user_feature_sources,
            solver::{AssignData, FeatState, outer_group, scan_arg_range, solve_all},
        },
        feature::{FeatureDefinition, ReplaceWith},
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

#[derive(Debug, Clone, PartialEq)]
pub enum RebuildError {
    MissingDefinition {
        kind: DefinitionKind,
        name: Box<str>,
    },
    MulticlassPrereq {
        class: Box<str>,
    },
}

/// `build_clean` result: rebuilt character + per-feature drift accounting.
#[derive(Debug, Clone)]
pub struct RebuildOutcome {
    pub character: Character,
    /// User-source features with missing definitions, kept as orphans.
    pub skipped_features: Vec<String>,
    /// Identity-source features with missing definitions, dropped.
    pub removed_features: Vec<String>,
}

/// Skipped/removed accumulator threaded through the apply pipeline.
#[derive(Default)]
pub struct RebuildAccum {
    pub skipped: Vec<String>,
    pub removed: Vec<String>,
}

/// State for the orphan-preservation path: the index dispatches known
/// vs unknown, `original` is the source-of-truth for missing-def User
/// feats looked up by Nth occurrence, `accum` accumulates skipped/removed
/// names for the rebuild outcome, and `user_seen` keeps a stable Nth
/// counter across duplicate empty-name placeholders.
pub struct RebuildCtx<'a> {
    pub feat_index: FeaturesView<'a>,
    pub original: &'a Character,
    pub accum: &'a mut RebuildAccum,
    pub user_seen: BTreeMap<(String, FeatureSource), usize>,
}

/// Output of `prepare_rebuild`: everything the rebuild caller needs to drive
/// silent-commit or the args modal.
pub struct RebuildPreview {
    /// Reconciled original character — feeds into `build_clean`.
    pub original: Character,
    /// Canonical plan walked by `build_clean`. Built once in `prepare_rebuild`
    /// and threaded through to avoid re-derivation.
    pub plan: Vec<PendingFeature>,
    /// Feats the modal cascade walks, in pipeline order. Starts at the first
    /// unsolved / rejected feat; `hidden=true` entries between editable
    /// ones are effective-stored feats the cascade applies silently.
    pub pending: Vec<PendingInputs>,
    /// Cascade seed: identity + every effective-stored feat applied in
    /// pipeline order up to (but not including) the first emitted feat.
    /// The modal layers `pending` on top of this, so `expr.analyze` on the
    /// first editable feat sees a correct baseline out of the gate.
    pub cascade_base: CharacterCore,
    /// `true` when an interactive feat lacks usable stored inputs in the
    /// original (rejected as ineffective, or simply empty) — forces the
    /// modal to open even if `build_clean` would silent-match on
    /// solver-derived zeros.
    pub had_rejections: bool,
}

/// Snapshot-level step 1: reconcile User-sourced features against identity
/// slots, run a dry-run cascade to discover what will end up in the rebuilt
/// character, then collect pending inputs that need user interaction.
///
/// The dry-run uses the same `build_clean` path the real apply takes — no
/// parallel discovery logic that can drift from cascade semantics.
#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "rebuild.prepare", skip_all)
)]
pub fn prepare_rebuild(
    mut original: Character,
    registry: &RulesRegistry,
) -> Result<RebuildPreview, RebuildError> {
    reconcile_user_feature_sources(&mut original, registry);
    let plan = level_up_plan(&original.identity, &original.features, registry)?;

    // Dry-run with empty inputs: cascade resolves identity-changing assigns
    // canonically and emits follow-ups via the same path the real rebuild
    // will take. The result's features.list is the authoritative
    // post-rebuild feature roster.
    let dry_run = build_clean(&original, &plan, registry, &ApplyInputs::default())?;
    let all_pending = features_to_pending(&dry_run.character.features.list);

    let (pending, had_rejections, cascade_base) =
        collect_rebuild_pending_inputs(&original, &all_pending, registry);
    Ok(RebuildPreview {
        original,
        plan,
        pending,
        cascade_base,
        had_rejections,
    })
}

/// Speculative recompute closure for the rebuild modal cascade. Runs a
/// dry-run `build_clean` against `original` to discover the post-rebuild
/// feature list; speculative supplies per-section prefill via
/// `pending_inputs(_, speculative)` so the user's in-modal picks land in the
/// prefill values.
pub fn rebuild_recompute(registry: RulesRegistry, original: &Character) -> RecomputePending {
    // Discovery (plan + dry-run) is deterministic given the captured
    // `original` and frozen registry — run once at construction. Per-tick
    // recompute only filters `all_pending` against `speculative` for prefill
    // lookups; on a L20 multiclass that saves a full cascade per picker tick.
    let all_pending = match level_up_plan(&original.identity, &original.features, &registry)
        .and_then(|plan| build_clean(original, &plan, &registry, &ApplyInputs::default()))
    {
        Ok(outcome) => features_to_pending(&outcome.character.features.list),
        Err(error) => {
            log::warn!("rebuild_recompute: discovery failed: {error:?}");
            Vec::new()
        }
    };
    Box::new(move |speculative: &CharacterCore| {
        registry.with_features_index_untracked(|feat_index| {
            all_pending
                .iter()
                .filter_map(|pending| {
                    let feat_def = feat_index.get(&pending.name)?;
                    pending.pending_inputs(feat_def, speculative)
                })
                .collect()
        })
    })
}

/// Convert the dry-run's `features.list` back into a `PendingFeature` list
/// in pipeline order. `applied` and `inputs` are intentionally dropped — they
/// live on the `original` and are looked up per-pending downstream.
fn features_to_pending(features: &[Feature]) -> Vec<PendingFeature> {
    features
        .iter()
        .map(|feature| PendingFeature {
            name: feature.name.clone(),
            source: feature.source.clone(),
            level: feature.source.added_at_level(),
            replaces: feature.replaces.clone(),
        })
        .collect()
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
    plan: &[PendingFeature],
    registry: &RulesRegistry,
    extra_inputs: &ApplyInputs,
) -> Result<RebuildOutcome, RebuildError> {
    let mut clean = rebuild_skeleton(original);
    let mut accum = RebuildAccum::default();

    registry.with_definitions(|caches| {
        registry.with_features_index_untracked(|feat_index| -> Result<(), RebuildError> {
            let mut ctx = RebuildCtx {
                feat_index,
                original,
                accum: &mut accum,
                user_seen: BTreeMap::new(),
            };

            // Pending keys span the entire plan — `detect_replacement`
            // needs to know which plan entries are also being applied so a
            // candidate that's already on the plan isn't mistaken for a
            // free-floating swap. Source borrows from `plan`, which
            // outlives the cascade calls below.
            let pending_keys: BTreeSet<(&str, &FeatureSource)> = plan
                .iter()
                .map(|pending| (&*pending.name, &pending.source))
                .collect();

            // Closures over Copy bits of ctx so cascade() can read while
            // ctx still hands out `&mut` for apply_user_pending below.
            let inputs_for = make_inputs_for(feat_index, original, extra_inputs);
            let replacement_for =
                make_replacement_for(feat_index, original, extra_inputs, &pending_keys);

            // Cascade the User(0) prefix as one slice — single-pending
            // cascade would interleave Species follow-ups between Human and
            // Noble, tearing the User(0) block apart.
            let base_end = plan
                .iter()
                .position(|pending| !matches!(pending.source, FeatureSource::User(0)))
                .unwrap_or(plan.len());
            cascade(
                &mut clean,
                &plan[..base_end],
                feat_index,
                caches,
                &inputs_for,
                &replacement_for,
                false,
            );

            for pending in &plan[base_end..] {
                if feat_index.contains_key(&pending.name) {
                    cascade(
                        &mut clean,
                        std::slice::from_ref(pending),
                        feat_index,
                        caches,
                        &inputs_for,
                        &replacement_for,
                        false,
                    );
                } else {
                    apply_user_pending(&mut ctx, &mut clean, pending);
                }
            }

            // Legacy migration: characters built before the Generation-feature
            // system have abilities edited directly. Synthesize a user feat so
            // future rebuilds keep them intact.
            migrate_legacy_abilities(&mut clean, original);

            // compute creates the empty SPELL.READY/KNOWN slots that
            // restore_all_spell_selections fills from the original.
            compute(&mut clean, ctx.feat_index);

            Ok(())
        })
    })?;

    merge_preserved(&mut clean, original);
    // Fresh character has empty Feature/Spell/Field labels — fill them
    // before commit so the UI doesn't briefly show structural English names
    // until the next locale-driven layout Effect run.
    registry.fill_from_registry(&mut clean);
    Ok(RebuildOutcome {
        character: clean,
        skipped_features: accum.skipped,
        removed_features: accum.removed,
    })
}

/// Skeleton character for rebuild walks: preserves stable identity (id,
/// name, XP) but drops fields the System markers re-derive (species,
/// background, classes). Avoids double-stacking `CLASS.LEVEL += 1` when
/// `original` already has class entries.
fn rebuild_skeleton(original: &Character) -> Character {
    let mut skeleton = Character {
        id: original.id,
        ..Character::default()
    };
    skeleton.personality.name = original.personality.name.clone();
    skeleton.identity.experience_points = original.identity.experience_points;
    skeleton
}

/// Preserve a User-source pending entry whose definition is missing from
/// `feat_index`. Caller (`build_clean`) routes known-def entries through
/// `cascade` directly; this function handles orphans only. Mirrors the
/// per-feature behavior of the legacy `apply_user_features_at_level`
/// (preserve missing-def User feats with a count). `user_seen` tracks the
/// Nth occurrence of each (name, source) so duplicate empty-name
/// placeholders all land in `clean`.
fn apply_user_pending(ctx: &mut RebuildCtx<'_>, clean: &mut Character, pending: &PendingFeature) {
    debug_assert!(
        !ctx.feat_index.contains_key(&pending.name),
        "apply_user_pending: known-def feat must go through cascade"
    );
    let key = (pending.name.to_string(), pending.source.clone());
    let occurrence = ctx.user_seen.entry(key).or_insert(0);
    let nth = *occurrence;
    *occurrence += 1;
    let Some(original_feature) = ctx
        .original
        .features
        .iter()
        .filter(|feature| {
            *feature.name == *pending.name
                && feature.source == pending.source
                && feature.source.is_user()
        })
        .nth(nth)
    else {
        return;
    };
    if !original_feature.name.is_empty() {
        log::warn!("rebuild: preserving user feature with no definition: {original_feature:?}");
        ctx.accum.skipped.push(original_feature.name.to_string());
    }
    clean.features.push(original_feature.clone());
}

/// Walk the dry-run-derived `all_pending` to identify which feats need user
/// input (OnFeatureAdd interactive exprs and no stored inputs in the
/// original). Stored-input validation rejects entries that no longer fit the
/// current derived state (`had_rejections`), forcing the modal open even when
/// silent-commit would otherwise match.
fn collect_rebuild_pending_inputs(
    original: &Character,
    all_pending: &[PendingFeature],
    registry: &RulesRegistry,
) -> (Vec<PendingInputs>, bool, CharacterCore) {
    // Returns (pending, had_rejections, cascade_base).
    registry.with_definitions(|caches| {
        registry.with_features_index_untracked(|feat_index| {
            // Precompute keys of every pending (name, source) so
            // `detect_replacement` can tell "not in the rebuilt list" from "in
            // the list under a different slot". Borrows from `all_pending` which
            // outlives this closure's inputs list.
            let pending_keys: BTreeSet<(&str, &FeatureSource)> = all_pending
                .iter()
                .map(|pending| (&*pending.name, &pending.source))
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
            // for its dry-run diff; the clone grows every iteration → O(N² ×
            // feature_count) on L20. Same in `solver::candidate_changes_baseline`.
            // Proper fix needs a derived-only clone, deferred to avoid breaking
            // a future spells-through-assign refactor (may write to features.data).
            let mut validation_baseline = rebuild_skeleton(original);
            let mut cascade_base: Option<CharacterCore> = None;
            let mut feat_states: Vec<FeatState> = Vec::new();
            let mut state_idx_by_pending: Vec<Option<usize>> =
                Vec::with_capacity(all_pending.len());
            let mut had_rejections = false;
            for pending in all_pending {
                let Some(feat_def) = feat_index.get(&pending.name) else {
                    state_idx_by_pending.push(None);
                    continue;
                };
                let Some(assign_defs) = feat_def.assign.as_ref() else {
                    // Pure-replaceable feats (e.g. Versatile: replace_with=Origin,
                    // no assign) emit in the modal so the user can pick. Capture
                    // cascade_base here if no interactive feat has done so yet —
                    // nothing before advanced validation_baseline for this branch,
                    // so the base matches.
                    let needs_modal = !matches!(feat_def.replace_with, ReplaceWith::None);
                    if needs_modal && cascade_base.is_none() {
                        cascade_base = Some(validation_baseline.core.clone());
                    }
                    state_idx_by_pending.push(None);
                    continue;
                };
                let has_interactive_onadd = assign_defs
                    .iter()
                    .any(|asn| asn.when == WhenCondition::OnFeatureAdd && asn.is_interactive());
                if has_interactive_onadd && cascade_base.is_none() {
                    cascade_base = Some(validation_baseline.core.clone());
                }
                let stored = original.features.get_inputs(&pending.name, &pending.source);
                let effective = !has_interactive_onadd
                    || stored_inputs_effective(
                        feat_def,
                        pending.level,
                        &validation_baseline,
                        stored,
                        caches,
                    );
                if has_interactive_onadd && !effective {
                    // Either corrupt stored (e.g. Expertise on now non-proficient
                    // skills) or no stored at all (fresh interactive feat). Both
                    // cases must surface the modal — silent-commit with solver
                    // zeros would bypass the user.
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
                        .filter(|asn| {
                            asn.when == WhenCondition::OnFeatureAdd && asn.is_interactive()
                        })
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
                    dry_run_apply_feature(
                        feat_def,
                        &mut validation_baseline,
                        pending,
                        stored,
                        WhenCondition::OnFeatureAdd,
                        caches,
                    );
                }
            }
            // No interactive feats → caller short-circuits via `pending.is_empty()`;
            // fallback seed is the fully-advanced validation_baseline.
            let cascade_base = cascade_base.unwrap_or_else(|| validation_baseline.core.clone());

            // Solver starts from identity (it re-applies every feat through
            // its own pipeline walk). Ignore the success flag — even a partial
            // solve leaves the best-effort prefill in each assign's `args`.
            let solver_baseline = rebuild_skeleton(original);
            let _ = solve_all(
                &mut feat_states,
                &solver_baseline.core,
                &original.core,
                caches,
            );

            // Emit: walk all_pending. Interactive feats emit editable from
            // their solver-solved args; non-interactive with `assign` ride as
            // hidden once the visible section has opened. `emit_baseline`
            // advances through every applied feat so `detect_replacement`
            // sees the pre-apply state for each.
            let mut emit_baseline = rebuild_skeleton(original);
            let mut inputs: Vec<PendingInputs> = Vec::new();
            let mut emit_started = false;
            for (pending_idx, pending) in all_pending.iter().enumerate() {
                let Some(feat_def) = feat_index.get(&pending.name) else {
                    continue;
                };
                if let Some(state_idx) = state_idx_by_pending[pending_idx] {
                    let state = &feat_states[state_idx];
                    // Solver resolves args; dice are rolled values that live
                    // only in the original's stored inputs — preserve them so
                    // a Random-generation rebuild doesn't ask the user to
                    // re-roll.
                    let stored = original.features.get_inputs(&pending.name, &pending.source);
                    let inputs_vec: Vec<AssignInputs> = state
                        .assigns
                        .iter()
                        .enumerate()
                        .map(|(idx, assign)| AssignInputs {
                            args: assign.args.clone(),
                            dice: stored
                                .get(idx)
                                .map(|input| input.dice.clone())
                                .unwrap_or_default(),
                        })
                        .collect();

                    let prefilled_replacement = detect_replacement(
                        &state.pending.feature_key(),
                        state.def,
                        original,
                        feat_index,
                        &pending_keys,
                        &emit_baseline,
                    );

                    dry_run_apply_feature(
                        state.def,
                        &mut emit_baseline,
                        state.pending,
                        &inputs_vec,
                        WhenCondition::OnFeatureAdd,
                        caches,
                    );

                    if let Some(mut pending_input) =
                        state.pending.pending_inputs(state.def, original)
                    {
                        pending_input.prefill = inputs_vec;
                        pending_input.prefilled_replacement = prefilled_replacement;
                        inputs.push(pending_input);
                    }
                    emit_started = true;
                } else if feat_def.assign.is_some() {
                    // apply_pending (not bare feat_def.apply) so the row
                    // lands in features.list — caster_info reads it.
                    apply_pending(feat_index, &mut emit_baseline, pending, &[], caches, false);
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
                        &pending.feature_key(),
                        feat_def,
                        original,
                        feat_index,
                        &pending_keys,
                        &emit_baseline,
                    );
                    if let Some(mut pending_input) = pending.pending_inputs(feat_def, original) {
                        pending_input.prefilled_replacement = prefilled_replacement;
                        inputs.push(pending_input);
                    }
                    emit_started = true;
                }
            }
            (inputs, had_rejections, cascade_base)
        })
    })
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
    level: u32,
    baseline: &Character,
    stored: &[AssignInputs],
    caches: DefinitionCaches,
) -> bool {
    if !stored_inputs_usable(feat_def, stored) {
        return false;
    }
    let mut trial = baseline.clone();
    let pending = PendingFeature {
        name: feat_def.name.clone(),
        source: FeatureSource::User(level),
        level,
        replaces: None,
    };
    dry_run_apply_feature(
        feat_def,
        &mut trial,
        &pending,
        stored,
        WhenCondition::OnFeatureAdd,
        caches,
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
            name: name.into(),
            label: None,
            description: String::new(),
            applied: true,
            category: FeatureCategory::Generation,
            source: FeatureSource::User(0),
            inputs: vec![AssignInputs {
                args,
                ..AssignInputs::default()
            }],
            replaces: None,
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

/// Build a `cascade()`-compatible `inputs_for` closure that resolves any
/// `FeatureKey` against `original`'s stored inputs and the modal-supplied
/// `extra_inputs`. Encapsulates the stackable lookup + proxy construction
/// every rebuild walker would otherwise spell out by hand.
pub fn make_inputs_for<'a>(
    feat_index: FeaturesView<'a>,
    original: &'a Character,
    extra_inputs: &'a ApplyInputs,
) -> impl Fn(&FeatureKey) -> Vec<AssignInputs> + 'a {
    move |key| {
        let stackable = feat_index
            .get(&key.name)
            .map(|definition| definition.stackable)
            .unwrap_or(false);
        let proxy = PendingFeature {
            name: key.name.clone(),
            source: key.source.clone(),
            level: key.source.added_at_level(),
            replaces: None,
        };
        inputs_for_pending(&proxy, original, extra_inputs, stackable)
    }
}

/// Build a `cascade()`-compatible `replacement_for` closure. Presence in
/// `extra_inputs` is authoritative (including `replacement = None` for an
/// explicit uncheck); only absent keys fall back to `detect_replacement`
/// against `original.features` for prior-swap recovery.
pub fn make_replacement_for<'a>(
    feat_index: FeaturesView<'a>,
    original: &'a Character,
    extra_inputs: &'a ApplyInputs,
    pending_keys: &'a BTreeSet<(&str, &FeatureSource)>,
) -> impl Fn(&FeatureKey) -> Option<Box<str>> + 'a {
    move |key| {
        if let Some(input) = extra_inputs.get(key) {
            return input.replacement.as_deref().map(Box::from);
        }
        let feat_def = feat_index.get(&key.name)?;
        detect_replacement(
            key,
            feat_def,
            &original.core,
            feat_index,
            pending_keys,
            &original.core,
        )
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
pub fn inputs_for_pending(
    pending_feature: &PendingFeature,
    original: &Character,
    extra_inputs: &ApplyInputs,
    stackable: bool,
) -> Vec<AssignInputs> {
    let key = FeatureKey::from_pending(pending_feature);
    if let Some(input) = extra_inputs.get(&key)
        && !input.inputs.is_empty()
    {
        return input.inputs.clone();
    }
    original
        .features
        .iter()
        .find(|feature| {
            *feature.name == *pending_feature.name
                && feature.applied
                && (!stackable || feature.source == pending_feature.source)
        })
        .map(|feature| feature.inputs.clone())
        .unwrap_or_default()
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
            AssignInputs, CharacterCore, ClassLevel, CombatStats, Die, Feature, FeatureData,
            FeatureField, FeatureValue, IdentitySlot, Note, Personality, ProficiencyLevel, Skill,
        },
        rules::apply::pending::ApplyInput,
    };

    fn feature(name: &str, source: FeatureSource) -> Feature {
        Feature {
            name: name.into(),
            source,
            applied: true,
            ..Feature::default()
        }
    }

    #[wasm_bindgen_test]
    fn merge_preserves_equipment_personality_notes() {
        let original = Character {
            personality: Personality {
                history: "backstory".into(),
                ..Default::default()
            },
            notes: vec![Note {
                created_at: 42,
                level: 3,
                text: "important notes".into(),
            }],
            ..Default::default()
        };

        let mut clean = Character::default();
        merge_preserved(&mut clean, &original);

        assert_eq!(clean.notes.len(), 1);
        assert_eq!(clean.notes[0].text, "important notes");
        assert_eq!(clean.notes[0].level, 3);
        assert_eq!(clean.personality.history, "backstory");
    }

    #[wasm_bindgen_test]
    fn merge_preserves_hp_current_and_death_saves() {
        let original = Character {
            core: CharacterCore {
                combat: CombatStats {
                    hp_current: 7,
                    hp_temp: 3,
                    death_save_successes: 2,
                    death_save_failures: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let mut clean = Character {
            core: CharacterCore {
                combat: CombatStats {
                    hp_max: 20,
                    hp_current: 20,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

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
                replaces: None,
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
                replaces: None,
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
        extra.insert(
            FeatureKey::new("Mystery", FeatureSource::User(0)),
            ApplyInput {
                inputs: modal_inputs.clone(),
                replacement: None,
            },
        );

        let inputs = inputs_for_pending(
            &PendingFeature {
                name: "Mystery".into(),
                source: FeatureSource::User(0),
                level: 0,
                replaces: None,
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
        extra.insert(
            FeatureKey::new("Mystery", FeatureSource::User(0)),
            ApplyInput {
                inputs: modal_inputs.clone(),
                replacement: None,
            },
        );

        let inputs = inputs_for_pending(
            &PendingFeature {
                name: "Mystery".into(),
                source: FeatureSource::User(0),
                level: 0,
                replaces: None,
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
            actions: BTreeMap::new(),
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
        feat_index.insert(slot_def.name.clone(), slot_def.clone());
        feat_index.insert(swap_def.name.clone(), swap_def.clone());

        let mut original = Character::default();
        original
            .features
            .list
            .push(feature("Arcane Trickster", slot_source.clone()));

        let pending = PendingFeature {
            name: "Rogue Subclass".into(),
            source: slot_source.clone(),
            level: 3,
            replaces: None,
        };
        let pending_keys: BTreeSet<(&str, &FeatureSource)> =
            [(&*pending.name, &pending.source)].into_iter().collect();
        let baseline = Character::default();

        let found = detect_replacement(
            &pending.feature_key(),
            &slot_def,
            &original,
            FeaturesView::from_natural(&feat_index),
            &pending_keys,
            &baseline,
        );
        assert_eq!(found, Some("Arcane Trickster".into()));
    }

    #[wasm_bindgen_test]
    fn detect_replacement_fast_path_wins_over_fallback_heuristic() {
        // Two features at the slot's source: A carries `replaces = Some(slot)`,
        // B is registered in the index with a matching category and would win
        // the legacy heuristic. Fast path must return A.
        let slot_source = FeatureSource::Class("Fighter".into(), 4);
        let slot_def = feat_def("ASI", FeatureCategory::Class, ReplaceWith::Any);
        let fallback_pick = feat_def("Tough", FeatureCategory::Class, ReplaceWith::None);

        let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index.insert(slot_def.name.clone(), slot_def.clone());
        feat_index.insert(fallback_pick.name.clone(), fallback_pick.clone());

        let mut original = Character::default();
        original
            .features
            .list
            .push(feature("Tough", slot_source.clone()));
        let mut explicit = feature("Lucky", slot_source.clone());
        explicit.replaces = Some("ASI".into());
        original.features.list.push(explicit);

        let pending = PendingFeature {
            name: "ASI".into(),
            source: slot_source.clone(),
            level: 4,
            replaces: None,
        };
        let pending_keys: BTreeSet<(&str, &FeatureSource)> =
            [(&*pending.name, &pending.source)].into_iter().collect();
        let baseline = Character::default();

        let found = detect_replacement(
            &pending.feature_key(),
            &slot_def,
            &original,
            FeaturesView::from_natural(&feat_index),
            &pending_keys,
            &baseline,
        );
        assert_eq!(found, Some("Lucky".into()));
    }

    #[wasm_bindgen_test]
    fn detect_replacement_uses_explicit_replaces_field() {
        // Fast path: a feature carrying `replaces = Some(<placeholder>)` wins
        // even when the swap def is missing from the index — the legacy
        // heuristic needs the def to test category/prereqs, the new field
        // doesn't.
        let slot_source = FeatureSource::Class("Rogue".into(), 3);
        let slot_def = feat_def("Rogue Subclass", FeatureCategory::Class, ReplaceWith::Any);

        let feat_index: BTreeMap<Box<str>, FeatureDefinition> =
            std::iter::once((slot_def.name.clone(), slot_def.clone())).collect();

        let mut original = Character::default();
        let mut swap = feature("Arcane Trickster", slot_source.clone());
        swap.replaces = Some("Rogue Subclass".into());
        original.features.list.push(swap);

        let pending = PendingFeature {
            name: "Rogue Subclass".into(),
            source: slot_source.clone(),
            level: 3,
            replaces: None,
        };
        let pending_keys: BTreeSet<(&str, &FeatureSource)> =
            [(&*pending.name, &pending.source)].into_iter().collect();
        let baseline = Character::default();

        let found = detect_replacement(
            &pending.feature_key(),
            &slot_def,
            &original,
            FeaturesView::from_natural(&feat_index),
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
            std::iter::once((slot_def.name.clone(), slot_def.clone())).collect();

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
            replaces: None,
        };
        let pending_keys: BTreeSet<(&str, &FeatureSource)> =
            [(&*pending.name, &pending.source)].into_iter().collect();
        let baseline = Character::default();

        let found = detect_replacement(
            &pending.feature_key(),
            &slot_def,
            &original,
            FeaturesView::from_natural(&feat_index),
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
        let caches = DefinitionCaches::empty();
        assert!(!stored_inputs_effective(
            &expertise_def,
            1,
            &baseline,
            &stored,
            caches,
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
            1,
            &baseline_prof,
            &stored,
            caches,
        ));
    }

    /// Walk every User(level) feat in `original`, dispatching the same way
    /// `build_clean` does: known-def → cascade, missing-def → orphan
    /// preservation.
    fn run_apply_user_pending(
        original: &Character,
        clean: &mut Character,
        feat_index: &BTreeMap<Box<str>, FeatureDefinition>,
        accum: &mut RebuildAccum,
        level: u32,
    ) {
        let extra = ApplyInputs::default();
        let pendings: Vec<PendingFeature> = original
            .features
            .iter()
            .filter(|feature| matches!(&feature.source, FeatureSource::User(l) if *l == level))
            .map(|feature| PendingFeature {
                name: feature.name.clone(),
                source: feature.source.clone(),
                level,
                replaces: None,
            })
            .collect();
        let empty_caches = DefinitionCaches::empty();
        let feat_index = FeaturesView::from_natural(feat_index);
        let mut ctx = RebuildCtx {
            feat_index,
            original,
            accum,
            user_seen: BTreeMap::new(),
        };
        let pending_keys = BTreeSet::new();
        let inputs_for = make_inputs_for(feat_index, original, &extra);
        let replacement_for = make_replacement_for(feat_index, original, &extra, &pending_keys);
        for pending in &pendings {
            if feat_index.contains_key(&pending.name) {
                cascade(
                    clean,
                    std::slice::from_ref(pending),
                    feat_index,
                    empty_caches,
                    &inputs_for,
                    &replacement_for,
                    false,
                );
            } else {
                apply_user_pending(&mut ctx, clean, pending);
            }
        }
    }

    #[wasm_bindgen_test]
    fn apply_user_features_preserves_empty_name_silently() {
        // Empty-name User slots come from the "Add feature" button in the
        // build tab — the user clicked it but hasn't picked anything yet.
        // Rebuild must keep them around (otherwise the user's slot disappears)
        // and must not surface them as "skipped" — the user already knows.
        let mut original = Character::default();
        original.features.list.push(Feature {
            name: Box::default(),
            source: FeatureSource::User(0),
            applied: false,
            ..Feature::default()
        });
        original.features.list.push(Feature {
            name: Box::default(),
            source: FeatureSource::User(0),
            applied: false,
            ..Feature::default()
        });
        let mut clean = Character::default();
        let mut accum = RebuildAccum::default();
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();

        run_apply_user_pending(&original, &mut clean, &feat_index, &mut accum, 0);

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
        let mut clean = Character::default();
        let mut accum = RebuildAccum::default();
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();

        run_apply_user_pending(&original, &mut clean, &feat_index, &mut accum, 0);

        let preserved = clean
            .features
            .list
            .iter()
            .find(|feature| &*feature.name == "Generation: Custom")
            .expect("preserved in clean");
        assert_eq!(preserved.inputs, inputs, "user inputs survive");
        assert_eq!(accum.skipped, vec!["Generation: Custom".to_string()]);
        assert!(accum.removed.is_empty());
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
            name: Box::default(),
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
        let mut clean = Character::default();
        let mut accum = RebuildAccum::default();

        run_apply_user_pending(&original, &mut clean, &feat_index, &mut accum, 0);

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
                .any(|feature| &*feature.name == "Homebrew Smite")
        );
        assert!(
            clean
                .features
                .list
                .iter()
                .any(|feature| &*feature.name == "Known Feat" && feature.applied)
        );
    }

    #[wasm_bindgen_test]
    fn detect_replacement_returns_none_when_not_replaceable() {
        let slot_source = FeatureSource::Class("Rogue".into(), 3);
        let slot_def = feat_def("Cunning Action", FeatureCategory::Class, ReplaceWith::None);
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> =
            std::iter::once((slot_def.name.clone(), slot_def.clone())).collect();

        let original = Character::default();
        let pending = PendingFeature {
            name: "Cunning Action".into(),
            source: slot_source.clone(),
            level: 3,
            replaces: None,
        };
        let pending_keys: BTreeSet<(&str, &FeatureSource)> =
            [(&*pending.name, &pending.source)].into_iter().collect();
        let baseline = Character::default();

        assert!(
            detect_replacement(
                &pending.feature_key(),
                &slot_def,
                &original,
                FeaturesView::from_natural(&feat_index),
                &pending_keys,
                &baseline
            )
            .is_none()
        );
    }

    /// Source-aware fallback: an `original.features` row that explicitly
    /// `replaces` a placeholder at a specific source resolves only for
    /// that source — not for siblings of the same placeholder name at
    /// other sources. The user-reported case: ASI L4 vanilla + ASI L8
    /// swapped to Spell Sniper. After rebuild, L4 must remain ASI and
    /// only L8 must turn into Spell Sniper.
    #[wasm_bindgen_test]
    fn make_replacement_for_resolves_only_at_matching_source() {
        let asi_def = feat_def(
            "Ability Score Improvement",
            FeatureCategory::Class,
            ReplaceWith::Any,
        );
        let spell_sniper_def =
            feat_def("Spell Sniper", FeatureCategory::General, ReplaceWith::None);
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> = [&asi_def, &spell_sniper_def]
            .into_iter()
            .map(|def| (def.name.clone(), def.clone()))
            .collect();
        let view = FeaturesView::from_natural(&feat_index);

        let l4_source = FeatureSource::Class("Sorcerer".into(), 4);
        let l8_source = FeatureSource::Class("Sorcerer".into(), 8);

        let mut original = Character::default();
        // L4: vanilla ASI (no `replaces`).
        original.features.list.push(Feature {
            name: "Ability Score Improvement".into(),
            source: l4_source.clone(),
            category: FeatureCategory::Class,
            applied: true,
            inputs: vec![AssignInputs {
                args: vec![0, 1, 0, 0, 0, 1],
                ..AssignInputs::default()
            }],
            ..Feature::default()
        });
        // L8: Spell Sniper swap recorded via `replaces`.
        original.features.list.push(Feature {
            name: "Spell Sniper".into(),
            source: l8_source.clone(),
            category: FeatureCategory::General,
            applied: true,
            inputs: vec![AssignInputs {
                args: vec![0, 0, 1],
                ..AssignInputs::default()
            }],
            replaces: Some("Ability Score Improvement".into()),
            ..Feature::default()
        });

        let extra = ApplyInputs::default();
        let pending_keys = BTreeSet::new();
        let resolve = make_replacement_for(view, &original, &extra, &pending_keys);

        // L4: no swap stored at this source → the closure must NOT bleed
        // L8's pick across.
        assert_eq!(
            resolve(&FeatureKey::new("Ability Score Improvement", l4_source)),
            None,
            "L4 ASI should stay vanilla — no swap at this source"
        );

        // L8: explicit `replaces` chain → must resolve to Spell Sniper.
        assert_eq!(
            resolve(&FeatureKey::new("Ability Score Improvement", l8_source)),
            Some("Spell Sniper".into()),
            "L8 ASI should rebuild as Spell Sniper from stored replacement"
        );

        // Unrelated source (L12) without any matching record → None.
        assert_eq!(
            resolve(&FeatureKey::new(
                "Ability Score Improvement",
                FeatureSource::Class("Sorcerer".into(), 12),
            )),
            None,
            "L12 ASI has no stored swap — must not pick up L8's"
        );
    }

    /// Modal-supplied (`extra_inputs`) wins over `original.features`. The
    /// modal contract: a fresh user pick at submit time must override any
    /// prior stored decision at the same key.
    #[wasm_bindgen_test]
    fn make_replacement_for_extra_inputs_wins() {
        let asi_def = feat_def(
            "Ability Score Improvement",
            FeatureCategory::Class,
            ReplaceWith::Any,
        );
        let spell_sniper_def =
            feat_def("Spell Sniper", FeatureCategory::General, ReplaceWith::None);
        let lucky_def = feat_def("Lucky", FeatureCategory::General, ReplaceWith::None);
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> =
            [&asi_def, &spell_sniper_def, &lucky_def]
                .into_iter()
                .map(|def| (def.name.clone(), def.clone()))
                .collect();
        let view = FeaturesView::from_natural(&feat_index);

        let l8_source = FeatureSource::Class("Sorcerer".into(), 8);
        let l8_key = FeatureKey::new("Ability Score Improvement", l8_source.clone());

        let mut original = Character::default();
        original.features.list.push(Feature {
            name: "Spell Sniper".into(),
            source: l8_source.clone(),
            applied: true,
            replaces: Some("Ability Score Improvement".into()),
            ..Feature::default()
        });

        // User just submitted modal with Lucky picked instead.
        let mut extra = ApplyInputs::default();
        extra.insert(
            l8_key.clone(),
            ApplyInput {
                inputs: vec![],
                replacement: Some("Lucky".into()),
            },
        );

        let pending_keys = BTreeSet::new();
        let resolve = make_replacement_for(view, &original, &extra, &pending_keys);
        assert_eq!(
            resolve(&l8_key),
            Some("Lucky".into()),
            "modal submit must override prior stored swap"
        );
    }

    /// `replacement = None` in submit must override prior swap in `original`.
    #[wasm_bindgen_test]
    fn make_replacement_for_extra_inputs_explicit_none_wins() {
        let asi_def = feat_def(
            "Ability Score Improvement",
            FeatureCategory::Class,
            ReplaceWith::Any,
        );
        let spell_sniper_def =
            feat_def("Spell Sniper", FeatureCategory::General, ReplaceWith::None);
        let feat_index: BTreeMap<Box<str>, FeatureDefinition> = [&asi_def, &spell_sniper_def]
            .into_iter()
            .map(|def| (def.name.clone(), def.clone()))
            .collect();
        let view = FeaturesView::from_natural(&feat_index);

        let l4_source = FeatureSource::Class("Sorcerer".into(), 4);
        let l4_key = FeatureKey::new("Ability Score Improvement", l4_source.clone());

        let mut original = Character::default();
        original.features.list.push(Feature {
            name: "Spell Sniper".into(),
            source: l4_source.clone(),
            applied: true,
            replaces: Some("Ability Score Improvement".into()),
            ..Feature::default()
        });

        let mut extra = ApplyInputs::default();
        extra.insert(
            l4_key.clone(),
            ApplyInput {
                inputs: vec![AssignInputs {
                    args: vec![0, 1, 0, 0, 0, 1],
                    ..AssignInputs::default()
                }],
                replacement: None,
            },
        );

        let pending_keys = BTreeSet::new();
        let resolve = make_replacement_for(view, &original, &extra, &pending_keys);
        assert_eq!(resolve(&l4_key), None);
    }

    use crate::rules::{
        ClassIndexEntry,
        apply::plan::{plan_from_interleaving_with_caches, plan_from_markers},
        background::BackgroundDefinition,
        class::ClassDefinition,
        registry::make_system_feature,
        species::SpeciesDefinition,
    };

    /// Build a `feat_index` populated with synth System(_) defs for the named
    /// classes / subclasses / species / backgrounds plus any structural class
    /// features supplied through `extra`.
    fn synth_feat_index(
        classes: &[&str],
        subclasses: &[&str],
        species: &[&str],
        backgrounds: &[&str],
        extra: Vec<(&str, FeatureCategory)>,
    ) -> BTreeMap<Box<str>, FeatureDefinition> {
        let mut index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        for name in classes {
            index.insert(
                Box::from(*name),
                make_system_feature(Box::from(*name), IdentitySlot::Class),
            );
        }
        for name in subclasses {
            index.insert(
                Box::from(*name),
                make_system_feature(Box::from(*name), IdentitySlot::Subclass),
            );
        }
        for name in species {
            index.insert(
                Box::from(*name),
                make_system_feature(Box::from(*name), IdentitySlot::Species),
            );
        }
        for name in backgrounds {
            index.insert(
                Box::from(*name),
                make_system_feature(Box::from(*name), IdentitySlot::Background),
            );
        }
        for (name, category) in extra {
            index.insert(
                Box::from(name),
                FeatureDefinition {
                    name: Box::from(name),
                    stackable: false,
                    category,
                    replace_with: ReplaceWith::None,
                    spells: None,
                    actions: BTreeMap::new(),
                    assign: None,
                    prerequisites: None,
                },
            );
        }
        index
    }

    /// Walk a plan through cascade (mirroring `build_clean`'s loop) so tests
    /// can assert on post-walk shape without `merge_preserved`/label-fill.
    fn run_plan(
        plan: &[PendingFeature],
        original: &Character,
        feat_index: &BTreeMap<Box<str>, FeatureDefinition>,
        caches: DefinitionCaches,
    ) -> Character {
        let mut clean = Character::default();
        let extra = ApplyInputs::default();
        let mut accum = RebuildAccum::default();
        let feat_index = FeaturesView::from_natural(feat_index);
        let mut ctx = RebuildCtx {
            feat_index,
            original,
            accum: &mut accum,
            user_seen: BTreeMap::new(),
        };
        let pending_keys = BTreeSet::new();
        let inputs_for = make_inputs_for(feat_index, original, &extra);
        let replacement_for = make_replacement_for(feat_index, original, &extra, &pending_keys);
        for pending in plan {
            if feat_index.contains_key(&pending.name) {
                cascade(
                    &mut clean,
                    std::slice::from_ref(pending),
                    feat_index,
                    caches,
                    &inputs_for,
                    &replacement_for,
                    false,
                );
            } else {
                apply_user_pending(&mut ctx, &mut clean, pending);
            }
        }
        clean
    }

    fn fighter_class_def() -> ClassDefinition {
        serde_json::from_value(serde_json::json!({
            "name": "Fighter",
            "hit_die": 10,
            "levels": {
                "1": { "features": [] },
                "2": { "features": [] },
                "3": { "features": [] }
            },
            "subclasses": [
                {
                    "name": "Battle Master",
                    "levels": {
                        "3": { "features": [] }
                    }
                }
            ]
        }))
        .unwrap()
    }

    #[wasm_bindgen_test]
    fn rebuild_emits_system_class_markers_at_each_level() {
        let mut original = Character::default();
        original.identity.classes = vec![ClassLevel {
            class: "Fighter".into(),
            level: 3,
            ..ClassLevel::default()
        }];
        let class_entries: BTreeMap<Box<str>, ClassIndexEntry> = BTreeMap::new();
        let class_index: BTreeMap<Box<str>, ClassDefinition> =
            std::iter::once((Box::from("Fighter"), fighter_class_def())).collect();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };
        let feat_index = synth_feat_index(&["Fighter"], &["Battle Master"], &[], &[], vec![]);

        let plan = plan_from_interleaving_with_caches(
            &original.identity,
            &original.features,
            FeaturesView::from_natural(&feat_index),
            caches,
            &class_entries,
        )
        .expect("plan");

        let clean = run_plan(&plan, &original, &feat_index, caches);

        let class_markers: Vec<&Feature> = clean
            .features
            .list
            .iter()
            .filter(|feature| {
                &*feature.name == "Fighter"
                    && matches!(
                        feature.category,
                        FeatureCategory::System(IdentitySlot::Class)
                    )
            })
            .collect();
        assert_eq!(class_markers.len(), 3);
        assert_eq!(class_markers[0].source, FeatureSource::User(1));
        assert_eq!(class_markers[1].source, FeatureSource::User(2));
        assert_eq!(class_markers[2].source, FeatureSource::User(3));
    }

    #[wasm_bindgen_test]
    fn rebuild_emits_system_subclass_marker_at_pick_level() {
        let mut original = Character::default();
        original.identity.classes = vec![ClassLevel {
            class: "Fighter".into(),
            subclass: Some("Battle Master".into()),
            level: 3,
            ..ClassLevel::default()
        }];
        let class_entries: BTreeMap<Box<str>, ClassIndexEntry> = BTreeMap::new();
        let class_index: BTreeMap<Box<str>, ClassDefinition> =
            std::iter::once((Box::from("Fighter"), fighter_class_def())).collect();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };
        let feat_index = synth_feat_index(&["Fighter"], &["Battle Master"], &[], &[], vec![]);

        let plan = plan_from_interleaving_with_caches(
            &original.identity,
            &original.features,
            FeaturesView::from_natural(&feat_index),
            caches,
            &class_entries,
        )
        .expect("plan");
        let clean = run_plan(&plan, &original, &feat_index, caches);

        let subclass_markers: Vec<&Feature> = clean
            .features
            .list
            .iter()
            .filter(|feature| {
                &*feature.name == "Battle Master"
                    && matches!(
                        feature.category,
                        FeatureCategory::System(IdentitySlot::Subclass)
                    )
            })
            .collect();
        assert_eq!(subclass_markers.len(), 1);
        assert_eq!(
            subclass_markers[0].source,
            FeatureSource::Class("Fighter".into(), 3)
        );
    }

    #[wasm_bindgen_test]
    fn rebuild_emits_system_species_and_background_markers() {
        let mut original = Character::default();
        original.identity.species = "Human".into();
        original.identity.background = "Soldier".into();

        let class_entries: BTreeMap<Box<str>, ClassIndexEntry> = BTreeMap::new();
        let class_index: BTreeMap<Box<str>, ClassDefinition> = BTreeMap::new();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = std::iter::once((
            Box::from("Human"),
            serde_json::from_value::<SpeciesDefinition>(serde_json::json!({
                "name": "Human",
                "features": []
            }))
            .unwrap(),
        ))
        .collect();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = std::iter::once((
            Box::from("Soldier"),
            serde_json::from_value::<BackgroundDefinition>(serde_json::json!({
                "name": "Soldier",
                "features": []
            }))
            .unwrap(),
        ))
        .collect();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };
        let feat_index = synth_feat_index(&[], &[], &["Human"], &["Soldier"], vec![]);

        let plan = plan_from_interleaving_with_caches(
            &original.identity,
            &original.features,
            FeaturesView::from_natural(&feat_index),
            caches,
            &class_entries,
        )
        .expect("plan");
        let clean = run_plan(&plan, &original, &feat_index, caches);

        let species = clean
            .features
            .list
            .iter()
            .find(|feature| {
                &*feature.name == "Human"
                    && matches!(
                        feature.category,
                        FeatureCategory::System(IdentitySlot::Species)
                    )
            })
            .expect("species marker");
        assert_eq!(species.source, FeatureSource::User(0));

        let background = clean
            .features
            .list
            .iter()
            .find(|feature| {
                &*feature.name == "Soldier"
                    && matches!(
                        feature.category,
                        FeatureCategory::System(IdentitySlot::Background)
                    )
            })
            .expect("background marker");
        assert_eq!(background.source, FeatureSource::User(0));
    }

    #[wasm_bindgen_test]
    fn rebuild_legacy_char_round_trip_idempotent() {
        // Original = Fighter L3 with no System markers, just a Class-source
        // feature representing a class L1 grant. First rebuild emits markers.
        let mut original = Character::default();
        original.identity.classes = vec![ClassLevel {
            class: "Fighter".into(),
            level: 3,
            ..ClassLevel::default()
        }];
        original.applied.levels.insert("Fighter".into(), {
            let mut levels = crate::vecset::VecSet::new();
            levels.insert(1);
            levels.insert(2);
            levels.insert(3);
            levels
        });

        let class_entries: BTreeMap<Box<str>, ClassIndexEntry> = BTreeMap::new();
        let class_index: BTreeMap<Box<str>, ClassDefinition> =
            std::iter::once((Box::from("Fighter"), fighter_class_def())).collect();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };
        let feat_index = synth_feat_index(&["Fighter"], &["Battle Master"], &[], &[], vec![]);

        // First pass: legacy char → interleaving.
        assert!(plan_from_markers(&original.identity, &original.features).is_none());
        let plan1 = plan_from_interleaving_with_caches(
            &original.identity,
            &original.features,
            FeaturesView::from_natural(&feat_index),
            caches,
            &class_entries,
        )
        .expect("plan1");
        let clean1 = run_plan(&plan1, &original, &feat_index, caches);

        // Output now has 3 System(Class) markers; second rebuild must use
        // the marker path and produce the same shape.
        let plan2 =
            plan_from_markers(&clean1.identity, &clean1.features).expect("plan2 from markers");
        let clean2 = run_plan(&plan2, &clean1, &feat_index, caches);

        let class_markers_1: Vec<&FeatureSource> = clean1
            .features
            .list
            .iter()
            .filter(|feature| {
                matches!(
                    feature.category,
                    FeatureCategory::System(IdentitySlot::Class)
                )
            })
            .map(|feature| &feature.source)
            .collect();
        let class_markers_2: Vec<&FeatureSource> = clean2
            .features
            .list
            .iter()
            .filter(|feature| {
                matches!(
                    feature.category,
                    FeatureCategory::System(IdentitySlot::Class)
                )
            })
            .map(|feature| &feature.source)
            .collect();
        assert_eq!(class_markers_1, class_markers_2);
        assert_eq!(class_markers_2.len(), 3);
        assert!(!clean2.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn rebuild_marker_path_skips_interleaving() {
        // Markers populate identity.classes via the OnFeatureAdd assign
        // (CLASS.<name>.LEVEL += 1 auto-creates the ClassLevel row), so a
        // char with no identity.classes pre-populated still rebuilds cleanly.
        let mut original = Character::default();
        // identity.classes intentionally empty.
        original.features.list = vec![
            crate::model::Feature {
                name: "Fighter".into(),
                source: FeatureSource::User(1),
                applied: true,
                category: FeatureCategory::System(IdentitySlot::Class),
                ..crate::model::Feature::default()
            },
            crate::model::Feature {
                name: "Fighter".into(),
                source: FeatureSource::User(2),
                applied: true,
                category: FeatureCategory::System(IdentitySlot::Class),
                ..crate::model::Feature::default()
            },
            crate::model::Feature {
                name: "Fighter".into(),
                source: FeatureSource::User(3),
                applied: true,
                category: FeatureCategory::System(IdentitySlot::Class),
                ..crate::model::Feature::default()
            },
        ];

        let plan = plan_from_markers(&original.identity, &original.features).expect("marker plan");
        // Plan must contain exactly the three class steps (plus possible
        // species/background entries — none here).
        let class_steps = plan
            .iter()
            .filter(|pending| &*pending.name == "Fighter")
            .count();
        assert_eq!(class_steps, 3);

        let class_index: BTreeMap<Box<str>, ClassDefinition> =
            std::iter::once((Box::from("Fighter"), fighter_class_def())).collect();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };
        let feat_index = synth_feat_index(&["Fighter"], &["Battle Master"], &[], &[], vec![]);

        let clean = run_plan(&plan, &original, &feat_index, caches);

        // Markers populate identity.classes via assigns + cascade.
        let fighter = clean
            .identity
            .classes
            .iter()
            .find(|class_level| &*class_level.class == "Fighter")
            .expect("Fighter class created from markers");
        assert_eq!(fighter.level, 3);

        // Three System(Class) markers committed in features.list.
        let markers = clean
            .features
            .list
            .iter()
            .filter(|feature| {
                matches!(
                    feature.category,
                    FeatureCategory::System(IdentitySlot::Class)
                )
            })
            .count();
        assert_eq!(markers, 3);
    }

    #[wasm_bindgen_test]
    fn rebuild_collects_subclass_features_at_pick_level() {
        // Regression: Artificer 6 / Alchemist legacy char rebuilt without
        // markers used to drop the L3 subclass features (Alchemist Spells,
        // Experimental Elixir, Tools of the Trade) because the L3 System(Class)
        // step ran before clean.identity.classes had a subclass bound, and
        // System(Subclass) only applied the marker without collecting features.
        let class_def: ClassDefinition = serde_json::from_value(serde_json::json!({
            "name": "Fighter",
            "hit_die": 10,
            "levels": {
                "1": { "features": [] },
                "2": { "features": [] },
                "3": { "features": [] }
            },
            "subclasses": [
                {
                    "name": "Battle Master",
                    "levels": {
                        "3": { "features": ["Combat Superiority", "Maneuvers"] }
                    }
                }
            ]
        }))
        .unwrap();
        let class_entries: BTreeMap<Box<str>, ClassIndexEntry> = BTreeMap::new();
        let class_index: BTreeMap<Box<str>, ClassDefinition> =
            std::iter::once((Box::from("Fighter"), class_def)).collect();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };
        let feat_index = synth_feat_index(
            &["Fighter"],
            &["Battle Master"],
            &[],
            &[],
            vec![
                ("Combat Superiority", FeatureCategory::Class),
                ("Maneuvers", FeatureCategory::Class),
            ],
        );

        let mut original = Character::default();
        original.identity.classes = vec![ClassLevel {
            class: "Fighter".into(),
            subclass: Some("Battle Master".into()),
            level: 3,
            ..ClassLevel::default()
        }];

        let plan = plan_from_interleaving_with_caches(
            &original.identity,
            &original.features,
            FeaturesView::from_natural(&feat_index),
            caches,
            &class_entries,
        )
        .expect("plan");
        let clean = run_plan(&plan, &original, &feat_index, caches);

        let subclass_source = FeatureSource::Subclass("Fighter".into(), "Battle Master".into(), 3);
        for name in ["Combat Superiority", "Maneuvers"] {
            let feature = clean
                .features
                .iter()
                .find(|feature| &*feature.name == name)
                .unwrap_or_else(|| panic!("subclass feature {name} missing after rebuild"));
            assert_eq!(feature.source, subclass_source);
        }
    }
}
