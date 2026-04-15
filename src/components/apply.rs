use std::collections::{BTreeMap, VecDeque};

use leptos::prelude::*;
use reactive_stores::Store;
use strum::VariantArray;

use crate::{
    components::args_modal::ArgsModalCtx,
    expr,
    model::{
        Ability, AssignInputs, Attribute, Character, Feature, FeatureCategory, FeatureData,
        FeatureSource, FeatureValue, Spell, SpellData,
    },
    rules::{
        ApplyInputs, ClassDefinition, ClassIndexEntry, DefinitionStore, FeatureKey, PendingInputs,
        ReplaceWith, RulesRegistry, WhenCondition,
        apply::{
            PendingFeature, apply_new_features, collect_background_features,
            collect_class_features, collect_pending_features, collect_species_features,
            onlevelup_pass, reapply_existing, replay, resolve_replacements,
        },
        feature::FeatureDefinition,
    },
};

/// Collect all pending inputs (OnFeatureAdd for new features + OnLevelUp for
/// existing) from the given pending features list.
fn collect_all_inputs(
    store: &Store<Character>,
    registry: &RulesRegistry,
    pending: &[PendingFeature],
) -> Vec<PendingInputs> {
    registry.with_features_index_untracked(|fi| {
        let character = store.read_untracked();

        let new_inputs = pending.iter().filter_map(|pending_feature| {
            let feat_def = fi.get(pending_feature.name.as_str())?;
            pending_feature.pending_inputs(feat_def, &character)
        });

        let levelup_inputs =
            character
                .features
                .iter()
                .filter(|f| f.applied)
                .filter_map(|feature| {
                    let feat_def = fi.get(feature.name.as_str())?;
                    let exprs = feat_def.interactive_exprs(WhenCondition::OnLevelUp, &character);
                    (!exprs.is_empty()).then_some(PendingInputs {
                        feature_name: feature.name.clone(),
                        feature_label: feat_def.label().to_string(),
                        feature_description: feat_def.description.clone(),
                        exprs,
                        prefill: Vec::new(),
                        replace_with: ReplaceWith::None,
                        source: feature.source.clone(),
                    })
                });

        new_inputs.chain(levelup_inputs).collect()
    })
}

/// Top-level helper for the unified feature application pipeline.
/// Collects PendingInputs from pending features, shows the args modal if
/// needed, resolves replacements, calls the user callback, then computes.
pub fn apply_with_modal(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: Vec<PendingFeature>,
    callback: impl Fn(
        &mut Character,
        &[PendingFeature],
        &ApplyInputs,
        &BTreeMap<Box<str>, FeatureDefinition>,
    ) + Send
    + Sync
    + 'static,
) {
    let all_inputs = collect_all_inputs(&store, &registry, &pending);

    let apply = move |inputs: Option<&ApplyInputs>| {
        let empty = ApplyInputs::default();
        let inputs = inputs.unwrap_or(&empty);
        store.update(|character| {
            registry.with_features_index_untracked(|fi| {
                let resolved = resolve_replacements(&pending, &inputs.replacements, fi);
                callback(character, &resolved, inputs, fi);
            });
            registry.compute(character);
        });
    };

    if all_inputs.is_empty() {
        apply(None);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(all_inputs, move |inputs| apply(Some(&inputs)));
    }
}

/// Replay all applied features from scratch. Clones the character, resets
/// computed state, collects pending inputs on the clean clone (skipping
/// features with stored inputs), then either replays directly or shows the
/// args modal for features missing stored inputs.
pub fn replay_with_modal(store: Store<Character>, registry: RulesRegistry) {
    let mut clone = store.with_untracked(|character| character.clone());
    clone.reset_computed();

    let mut pending: Vec<PendingFeature> = clone
        .features
        .iter()
        .map(|f| PendingFeature {
            name: f.name.clone(),
            source: f.source.clone(),
            level: f.source.added_at_level(),
        })
        .collect();
    pending.sort_by_key(|p| p.source.added_at_level());

    let mut all_inputs = registry.with_features_index_untracked(|fi| {
        pending
            .iter()
            .filter_map(|pf| {
                let feat_def = fi.get(pf.name.as_str())?;
                pf.pending_inputs(feat_def, &clone)
            })
            .collect::<Vec<_>>()
    });
    all_inputs.retain(|input| clone.features.get_inputs(&input.feature_name).is_empty());

    let do_replay = move |inputs: Option<&ApplyInputs>| {
        let empty = ApplyInputs::default();
        let inputs = inputs.unwrap_or(&empty);
        store.update(|character| {
            let original_feature_data = character.feature_data.clone();
            *character = clone;
            registry.with_features_index_untracked(|fi| {
                replay(fi, character, &pending, inputs);
            });
            restore_all_spell_selections(&original_feature_data, &mut character.feature_data);
            registry.compute(character);
        });
    };

    if all_inputs.is_empty() {
        do_replay(None);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(all_inputs, move |inputs| do_replay(Some(&inputs)));
    }
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

/// Read-only context that resolves ARG variables from a slice for validation.
pub struct ArgsContext<'a> {
    pub character: &'a Character,
    pub args: &'a [i32],
}

impl expr::Context<Attribute, i32> for ArgsContext<'_> {
    fn assign(&mut self, _var: Attribute, _value: i32) -> Result<(), expr::Error> {
        Ok(())
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Arg(n) => self
                .args
                .get(n as usize)
                .copied()
                .ok_or_else(|| expr::Error::unsupported_var(var)),
            other => self.character.resolve(other),
        }
    }
}

/// Mutating context that simulates applying an assignment to a character
/// while also capturing every assign call (var, value) for display.
pub struct PreviewContext<'a> {
    pub character: &'a mut Character,
    pub captured: Vec<(Attribute, i32)>,
}

impl expr::Context<Attribute, i32> for PreviewContext<'_> {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        self.character.resolve(var)
    }

    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        if self
            .character
            .resolve(var)
            .ok()
            .is_none_or(|prev| prev != value)
        {
            self.captured.push((var, value));
        }
        self.character.assign(var, value)
    }
}

/// Like [`apply_with_modal`], but accepts pre-filled ARG values (e.g. from AI
/// generation). Features whose prefilled args validate successfully are applied
/// directly; any remaining features fall back to the interactive args modal.
pub fn apply_with_prefilled_args(
    store: Store<Character>,
    registry: RulesRegistry,
    pending: Vec<PendingFeature>,
    prefilled: BTreeMap<String, Vec<i32>>,
    prefilled_replacements: BTreeMap<String, String>,
    callback: impl Fn(
        &mut Character,
        &[PendingFeature],
        &ApplyInputs,
        &BTreeMap<Box<str>, FeatureDefinition>,
    ) + Send
    + Sync
    + 'static,
) {
    let all_inputs = collect_all_inputs(&store, &registry, &pending);

    // Partition inputs into pre-filled (validated) and fallback (needs modal)
    let mut validated_inputs = ApplyInputs::default();
    let mut fallback_names: Vec<String> = Vec::new();

    {
        let character = store.read_untracked();
        for pending_input in &all_inputs {
            // Check for prefilled replacement
            if pending_input.is_replaceable()
                && let Some(replacement) = prefilled_replacements.get(&pending_input.feature_name)
            {
                validated_inputs
                    .replacements
                    .insert(pending_input.feature_name.clone(), replacement.clone());
                // If the replacement has ARGs, validate them too
                if let Some(args) = prefilled.get(replacement) {
                    let expr_inputs = pending_input
                        .exprs
                        .iter()
                        .map(|_| AssignInputs {
                            args: args.clone(),
                            dice: Default::default(),
                        })
                        .collect();
                    validated_inputs.feature_inputs.insert(
                        FeatureKey::new(
                            pending_input.feature_name.clone(),
                            pending_input.source.clone(),
                        ),
                        expr_inputs,
                    );
                }
                continue;
            }

            if let Some(args) = prefilled.get(&pending_input.feature_name) {
                let all_valid = pending_input.exprs.iter().all(|expression| {
                    let ctx = ArgsContext {
                        character: &character,
                        args,
                    };
                    expression.eval_lenient(&ctx).is_ok()
                });

                if all_valid {
                    let expr_inputs = pending_input
                        .exprs
                        .iter()
                        .map(|_| AssignInputs {
                            args: args.clone(),
                            dice: Default::default(),
                        })
                        .collect();
                    validated_inputs.feature_inputs.insert(
                        FeatureKey::new(
                            pending_input.feature_name.clone(),
                            pending_input.source.clone(),
                        ),
                        expr_inputs,
                    );
                    continue;
                }
            }
            fallback_names.push(pending_input.feature_name.clone());
        }
    }

    // Features that have interactive assign expressions but weren't in
    // all_inputs (e.g. Expertise whose guard prunes all ARGs before
    // proficiencies are applied) also need to go to fallback.
    let inputs_names: Vec<_> = all_inputs
        .iter()
        .map(|input| input.feature_name.as_str())
        .collect();
    registry.with_features_index_untracked(|fi| {
        for pf in &pending {
            let already_validated = validated_inputs
                .feature_inputs
                .keys()
                .any(|key| key.name == pf.name);
            if !inputs_names.contains(&pf.name.as_str())
                && !already_validated
                && let Some(feat_def) = fi.get(pf.name.as_str())
            {
                let has_args = feat_def.assign.as_ref().is_some_and(|assigns| {
                    assigns.iter().any(|assignment| {
                        assignment
                            .expr
                            .has_var(|var| matches!(var, Attribute::Arg(_)))
                    })
                });
                if has_args {
                    fallback_names.push(pf.name.clone());
                }
            }
        }
    });

    if fallback_names.is_empty() {
        // All features validated — apply everything at once
        store.update(|character| {
            registry.with_features_index_untracked(|fi| {
                let resolved = resolve_replacements(&pending, &validated_inputs.replacements, fi);
                callback(character, &resolved, &validated_inputs, fi);
            });
            registry.compute(character);
        });
    } else {
        // Split: apply validated features first, then modal for the rest
        log::debug!("Fallback features needing modal: {fallback_names:?}");

        let validated_pending: Vec<_> = pending
            .iter()
            .filter(|pf| !fallback_names.contains(&pf.name))
            .cloned()
            .collect();
        let fallback_pending: Vec<_> = pending
            .into_iter()
            .filter(|pf| fallback_names.contains(&pf.name))
            .collect();

        // Apply validated features immediately
        store.update(|character| {
            registry.with_features_index_untracked(|fi| {
                let resolved =
                    resolve_replacements(&validated_pending, &validated_inputs.replacements, fi);
                callback(character, &resolved, &validated_inputs, fi);
            });
            registry.compute(character);
        });

        // Re-collect inputs for fallback features now that character state
        // has changed (e.g. Expertise needs proficiencies from Class Proficiencies)
        let refreshed_inputs: Vec<PendingInputs> = registry.with_features_index_untracked(|fi| {
            let character = store.read_untracked();
            fallback_pending
                .iter()
                .filter_map(|pending_feature| {
                    let feat_def = fi.get(pending_feature.name.as_str())?;
                    pending_feature.pending_inputs(feat_def, &character)
                })
                .collect()
        });

        // Show modal for remaining features
        if let Some(ctx) = use_context::<ArgsModalCtx>() {
            ctx.open(refreshed_inputs, move |modal_inputs| {
                store.update(|character| {
                    registry.with_features_index_untracked(|fi| {
                        let resolved =
                            resolve_replacements(&fallback_pending, &modal_inputs.replacements, fi);
                        callback(character, &resolved, &modal_inputs, fi);
                    });
                    registry.compute(character);
                });
            });
        } else {
            log::warn!("Skipping features without valid ARGs (no modal): {fallback_names:?}");
        }
    }
}

/// Collect pending features and apply them via modal.
pub fn apply_level(store: Store<Character>, registry: RulesRegistry) {
    let pending = store.with_untracked(|character| {
        registry
            .with_features_index_untracked(|fi| collect_pending_features(character, &registry, fi))
    });

    apply_with_modal(
        store,
        registry,
        pending,
        move |character, pending, inputs, fi| {
            // Mark species/background as applied if they had pending features
            if !character.identity.species_applied && !character.identity.species.is_empty() {
                character.identity.species_applied = true;
            }
            if !character.identity.background_applied && !character.identity.background.is_empty() {
                character.identity.background_applied = true;
            }
            // Mark class levels as applied
            let class_cache = registry.classes().cache().read_untracked();
            for class_level in &mut character.identity.classes {
                for lvl in 1..=class_level.level {
                    if !class_level.applied_levels.contains(&lvl) {
                        class_level.applied_levels.insert(lvl);
                    }
                }
                if let Some(def) = class_cache.get(class_level.class.as_str()) {
                    class_level.hit_die_sides = def.hit_die;
                }
            }

            reapply_existing(fi, character);
            apply_new_features(fi, character, pending, Some(inputs));
            character.combat.hp_current = character.hp_max();

            let xp_threshold = character.xp_threshold();
            if character.identity.experience_points < xp_threshold {
                character.identity.experience_points = xp_threshold;
            }
        },
    );
}

/// Apply a single class level only (used by per-level apply buttons).
pub fn apply_single_level(
    store: Store<Character>,
    registry: RulesRegistry,
    class_index: usize,
    level: u32,
) {
    let pending = store.with_untracked(|character| {
        let class_cache = registry.classes().cache().read_untracked();
        registry.with_features_index_untracked(|fi| {
            class_cache
                .get(character.identity.classes[class_index].class.as_str())
                .into_iter()
                .flat_map(|class_def| {
                    collect_class_features(character, class_index, level, class_def, fi)
                })
                .collect()
        })
    });

    apply_with_modal(
        store,
        registry,
        pending,
        move |character, pending, inputs, fi| {
            if let Some(class_level) = character.identity.classes.get_mut(class_index) {
                class_level.applied_levels.insert(level);
                registry.classes().with(&class_level.class, |def| {
                    class_level.hit_die_sides = def.hit_die;
                });
            }
            reapply_existing(fi, character);
            apply_new_features(fi, character, pending, Some(inputs));
            character.combat.hp_current = character.hp_max();
        },
    );
}

#[derive(Debug, Clone)]
pub enum RebuildError {
    MissingDefinition { kind: &'static str, name: String },
    MulticlassPrereq { class: String },
}

/// Transactional rebuild: constructs a clean `Character` from `default()`,
/// applies identity + User features in canonical order (User(0) → Species →
/// Background → classes round-robin by class level, with multiclass prereq
/// gates), runs `onlevelup_pass`, then merges the result back into the store
/// while preserving user in-game state (HP, used counters, spell selections,
/// equipment, personality, notes).
///
/// Opens the args modal if any feature scheduled for rebuild needs user
/// input (OnFeatureAdd interactive expressions) and has no stored inputs in
/// the original. Aborts with a browser alert on multiclass prereq failure
/// or missing class/species/background definitions — the store is never
/// written in that case.
pub fn rebuild(store: Store<Character>, registry: RulesRegistry) {
    let original = store.with_untracked(|character| character.clone());
    let pending_inputs = collect_rebuild_pending_inputs(&original, &registry);

    let do_rebuild = move |modal_inputs: Option<&ApplyInputs>| {
        let empty = ApplyInputs::default();
        let extra = modal_inputs.unwrap_or(&empty);
        match build_clean(&original, &registry, extra) {
            Ok(clean) => {
                store.update(|character| {
                    *character = clean;
                    registry.compute(character);
                });
            }
            Err(err) => show_rebuild_error(&err),
        }
    };

    if pending_inputs.is_empty() {
        do_rebuild(None);
    } else {
        let ctx = expect_context::<ArgsModalCtx>();
        ctx.open(pending_inputs, move |inputs| do_rebuild(Some(&inputs)));
    }
}

fn show_rebuild_error(err: &RebuildError) {
    let message = match err {
        RebuildError::MissingDefinition { kind, name } => {
            format!("Rebuild failed: missing {kind} definition '{name}'.")
        }
        RebuildError::MulticlassPrereq { class } => {
            format!("Rebuild failed: multiclass prerequisites for '{class}' never satisfied.")
        }
    };
    log::error!("{message}");
    window().alert_with_message(&message).ok();
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
        let mut snapshot = original.clone();
        snapshot
            .features
            .retain(|feature| matches!(feature.source, FeatureSource::User(_)));
        snapshot.identity.reset_applied_flags();
        let identity_pending = collect_pending_features(&snapshot, registry, fi);

        // User features from original — rebuild re-applies all of them.
        let user_pending = original
            .features
            .iter()
            .filter(|feature| matches!(feature.source, FeatureSource::User(_)))
            .map(|feature| PendingFeature {
                name: feature.name.clone(),
                source: feature.source.clone(),
                level: feature.source.added_at_level(),
            });

        let all_pending: Vec<PendingFeature> = user_pending.chain(identity_pending).collect();

        let mut inputs: Vec<PendingInputs> = all_pending
            .iter()
            .filter_map(|pf| {
                let feat_def = fi.get(pf.name.as_str())?;
                pf.pending_inputs(feat_def, original)
            })
            .collect();
        inputs.retain(|input| original.features.get_inputs(&input.feature_name).is_empty());
        inputs
    })
}

fn build_clean(
    original: &Character,
    registry: &RulesRegistry,
    extra_inputs: &ApplyInputs,
) -> Result<Character, RebuildError> {
    let mut clean = Character::default();
    clean.identity = original.identity.clone();
    clean.identity.reset_applied_flags();

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
            clean.identity.species_applied = true;
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
            clean.identity.background_applied = true;
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

/// If `original` has no feature in the `Generation` category (legacy
/// character pre-dating the generation-feature system), synthesize a
/// `Generation: Custom` User(0) feature with `inputs` set to the "base"
/// abilities (original scores minus identity contributions), prepend it to
/// `clean.features`, and bump `clean.abilities` to match `original`. On
/// the next rebuild the synthesized feature is applied first and the same
/// identity contributions stack on top, landing at `original.abilities`.
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
            clean.modify_ability(ability, orig - current);
        }
    }

    clean.features.insert(
        0,
        Feature {
            name: "Generation: Custom".to_string(),
            label: None,
            description: String::new(),
            applied: true,
            category: FeatureCategory::Generation,
            source: FeatureSource::User(0),
            inputs: vec![AssignInputs {
                args,
                dice: Default::default(),
            }],
        },
    );
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
        .find(|(i, (a, t))| a < t && !clean.identity.classes[*i].class.is_empty())
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
            .find(|(i, cl)| {
                !cl.class.is_empty() && applied[*i] < targets[*i] && meets_prereq(cl.class.as_str())
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
    clean.identity.classes[class_idx]
        .applied_levels
        .insert(class_level);
    Ok(())
}

/// Apply a batch of pending features. Replacements from the modal are
/// resolved upfront so the correct feature (after any user-chosen swap)
/// drives input lookup. Each resolved pending is applied individually:
/// `apply_new_features` looks up inputs in `ApplyInputs.feature_inputs`,
/// but the per-pending call carries only that one instance's inputs so
/// stackable features with the same name don't collide.
fn apply_pending(
    fi: &BTreeMap<Box<str>, FeatureDefinition>,
    clean: &mut Character,
    pending: &[PendingFeature],
    original: &Character,
    extra_inputs: &ApplyInputs,
) -> Result<(), RebuildError> {
    let resolved = resolve_replacements(pending, &extra_inputs.replacements, fi);
    for pending_feature in &resolved {
        if !fi.contains_key(pending_feature.name.as_str()) {
            return Err(RebuildError::MissingDefinition {
                kind: "feature",
                name: pending_feature.name.clone(),
            });
        }
        let inputs_for_one = inputs_for_pending(pending_feature, original, extra_inputs);
        apply_new_features(
            fi,
            clean,
            std::slice::from_ref(pending_feature),
            Some(&inputs_for_one),
        );
    }
    Ok(())
}

/// Resolve stored inputs for a single pending feature, matching by source so
/// stackable features (two ASI instances at different class levels, for
/// example) don't share storage. Falls back to `extra_inputs` (modal input)
/// when nothing stored.
fn inputs_for_pending(
    pending_feature: &PendingFeature,
    original: &Character,
    extra_inputs: &ApplyInputs,
) -> ApplyInputs {
    let stored: Vec<AssignInputs> = original
        .features
        .iter()
        .find(|feature| {
            feature.name == pending_feature.name
                && feature.applied
                && feature.source == pending_feature.source
        })
        .map(|feature| feature.inputs.clone())
        .unwrap_or_default();

    let key = FeatureKey::from_pending(pending_feature);
    let inputs = if !stored.is_empty() {
        stored
    } else {
        extra_inputs
            .feature_inputs
            .get(&key)
            .cloned()
            .unwrap_or_default()
    };

    let mut feature_inputs = BTreeMap::new();
    if !inputs.is_empty() {
        feature_inputs.insert(key, inputs);
    }
    ApplyInputs {
        feature_inputs,
        replacements: extra_inputs.replacements.clone(),
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

/// Restore user-selected spells from `original` feature_data into `clean`.
/// Iterates entries present in both, calling `restore_spell_selections` for
/// matching spells blocks. Shared by rebuild's `merge_preserved` and replay.
fn restore_all_spell_selections(
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
    // hp_max.
    clean.combat.hp_current = original.combat.hp_current.min(clean.combat.hp_max);
    clean.combat.hp_temp = original.combat.hp_temp;
    clean.combat.death_save_successes = original.combat.death_save_successes;
    clean.combat.death_save_failures = original.combat.death_save_failures;

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
    for (name, clean_data) in clean.feature_data.iter_mut() {
        let Some(orig_data) = original.feature_data.get(name) else {
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
    restore_all_spell_selections(&original.feature_data, &mut clean.feature_data);
}

#[cfg(test)]
mod rebuild_tests {
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
        original
            .feature_data
            .insert("Martial Arts".into(), orig_data);

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
        clean.feature_data.insert("Martial Arts".into(), clean_data);

        merge_preserved(&mut clean, &original);

        let data = clean.feature_data.get("Martial Arts").unwrap();
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
        original.feature_data.insert("Feat".into(), orig_data);

        let mut clean = Character::default();
        let mut clean_data = FeatureData::default();
        clean_data.fields.push(FeatureField {
            name: "Ki Points".into(),
            label: None,
            description: String::new(),
            value: FeatureValue::Points { used: 0, max: 3 },
        });
        clean.feature_data.insert("Feat".into(), clean_data);

        merge_preserved(&mut clean, &original);

        let used = match &clean.feature_data.get("Feat").unwrap().fields[0].value {
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
        original.features.push(Feature {
            inputs: asi4_inputs.clone(),
            ..feature(
                "Ability Score Improvement",
                FeatureSource::Class("Monk".into(), 4),
            )
        });
        original.features.push(Feature {
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
        );
        let inputs_at_8 = inputs_for_pending(
            &PendingFeature {
                name: "Ability Score Improvement".into(),
                source: FeatureSource::Class("Monk".into(), 8),
                level: 8,
            },
            &original,
            &empty,
        );

        assert_eq!(
            inputs_at_4.get(
                "Ability Score Improvement",
                &FeatureSource::Class("Monk".into(), 4)
            ),
            asi4_inputs.as_slice()
        );
        assert_eq!(
            inputs_at_8.get(
                "Ability Score Improvement",
                &FeatureSource::Class("Monk".into(), 8)
            ),
            asi8_inputs.as_slice()
        );
    }

    #[wasm_bindgen_test]
    fn inputs_for_pending_falls_back_to_extra_inputs_when_stored_empty() {
        let mut original = Character::default();
        original
            .features
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
        );

        assert_eq!(
            inputs.get("Mystery", &FeatureSource::User(0)),
            modal_inputs.as_slice()
        );
    }
}
