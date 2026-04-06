use std::collections::BTreeMap;

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::args_modal::ArgsModalCtx,
    expr,
    model::{AssignInputs, Attribute, Character},
    rules::{
        ApplyInputs, DefinitionStore, PendingInputs, ReplaceWith, RulesRegistry, WhenCondition,
        apply::{
            PendingFeature, apply_new_features, collect_class_features, collect_pending_features,
            reapply_existing, resolve_replacements,
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

/// Read-only context that resolves ARG variables from a slice for validation.
struct ArgsContext<'a> {
    character: &'a Character,
    args: &'a [i32],
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
