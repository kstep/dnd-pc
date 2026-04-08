use std::collections::{BTreeMap, VecDeque};

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::args_modal::ArgsModalCtx,
    expr,
    model::{AssignInputs, Attribute, Character, Spell, SpellData},
    rules::{
        ApplyInputs, DefinitionStore, PendingInputs, ReplaceWith, RulesRegistry, WhenCondition,
        apply::{
            PendingFeature, apply_new_features, collect_class_features, collect_pending_features,
            reapply_existing, replay, resolve_replacements,
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
        .filter(|f| f.applied)
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
            // Restore user spell selections from original character
            for (feature_name, original_data) in &original_feature_data {
                if let (Some(original_spells), Some(target_spells)) = (
                    &original_data.spells,
                    character
                        .feature_data
                        .get_mut(feature_name)
                        .and_then(|data| data.spells.as_mut()),
                ) {
                    restore_spell_selections(original_spells, target_spells);
                }
            }
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
                    validated_inputs
                        .feature_inputs
                        .insert(pending_input.feature_name.clone(), expr_inputs);
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
                    validated_inputs
                        .feature_inputs
                        .insert(pending_input.feature_name.clone(), expr_inputs);
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
            if !inputs_names.contains(&pf.name.as_str())
                && !validated_inputs.feature_inputs.contains_key(&pf.name)
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
