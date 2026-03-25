use std::collections::BTreeMap;

use leptos::prelude::*;

use super::{
    WhenCondition,
    resolve::{find_feature, find_feature_with_class_level},
    spells::SpellList,
};
use crate::{
    expr::Expr,
    model::{Attribute, Character, Context, FeatureSource, FeatureValue},
    rules::RulesRegistry,
};

/// A feature whose assignment expression requires user-supplied ARG values.
#[derive(Clone, PartialEq)]
pub struct PendingArgs {
    pub feature_name: String,
    pub feature_label: String,
    pub feature_description: String,
    pub expr: Expr<Attribute>,
}

impl RulesRegistry {
    pub fn long_rest(&self, character: &mut Character) {
        character.long_rest();
        self.assign(character, WhenCondition::OnLongRest);
    }

    pub fn short_rest(&self, character: &mut Character) {
        character.short_rest();
        self.assign(character, WhenCondition::OnShortRest);
    }

    pub fn compute(&self, character: &mut Character) {
        character.compute();
        self.assign(character, WhenCondition::OnCompute);
        self.recompute_dynamic_fields(character);
    }

    /// Re-evaluate dynamic field values (Points max, Die amount) after
    /// ability scores or other stats may have changed.
    fn recompute_dynamic_fields(&self, character: &mut Character) {
        self.with_features_index_untracked(|features_index| {
            let class_cache = self.class_cache.read_untracked();

            // Pre-compute dynamic values (needs &character for eval).
            // Collect (feat_name, field_index, new_value) — feat_name must be
            // owned to release the immutable borrow before the apply phase.
            let mut updates: Vec<(String, usize, FeatureValue)> = Vec::new();
            for (feat_name, entry) in &character.feature_data {
                let Some((feat_def, class_level)) = find_feature_with_class_level(
                    &character.identity,
                    feat_name,
                    features_index,
                    &class_cache,
                ) else {
                    continue;
                };
                for (i, field) in entry.fields.iter().enumerate() {
                    let Some(field_def) = feat_def.fields.get(field.name.as_str()) else {
                        continue;
                    };
                    if let Some(new_val) = field_def.kind.recompute_dynamic(class_level, character)
                    {
                        updates.push((feat_name.clone(), i, new_val));
                    }
                }
            }

            // Apply computed values by index
            for (feat_name, field_idx, new_val) in updates {
                if let Some(entry) = character.feature_data.get_mut(&feat_name)
                    && let Some(field) = entry.fields.get_mut(field_idx)
                {
                    match (&new_val, &mut field.value) {
                        (
                            FeatureValue::Points { max: new_max, .. },
                            FeatureValue::Points { max, .. },
                        ) => {
                            *max = *new_max;
                        }
                        (FeatureValue::Die { die: new_die, .. }, FeatureValue::Die { die, .. }) => {
                            *die = *new_die;
                        }
                        _ => {}
                    }
                }
            }
        });
    }

    /// Evaluate assignment expressions across all features for the given
    /// condition.
    ///
    /// Features are evaluated with per-feature `Context` providing
    /// `CLASS_LEVEL`, `CASTER_LEVEL`, and `CASTER_MODIFIER`.
    pub fn assign(&self, character: &mut Character, when: WhenCondition) {
        self.with_features_index_untracked(|features_index| {
            let class_cache = self.class_cache.read_untracked();

            // Collect per-feature info: (expressions, class_level, caster_level,
            // caster_modifier). Uses find_feature_with_class_level for a single-pass
            // lookup. The Vec is necessary because we need &mut character in the loop.
            let feature_entries: Vec<_> = character
                .features
                .iter()
                .filter_map(|feat| {
                    let (feat_def, class_level) = find_feature_with_class_level(
                        &character.identity,
                        &feat.name,
                        features_index,
                        &class_cache,
                    )?;
                    let exprs: Vec<_> = feat_def
                        .assign
                        .iter()
                        .flat_map(|a| a.iter())
                        .filter(|a| a.when == when)
                        .map(|a| a.expr.clone())
                        .collect();
                    if exprs.is_empty() {
                        return None;
                    }
                    let (caster_level, caster_modifier) = feat_def
                        .spells
                        .as_ref()
                        .map(|s| {
                            (
                                character.caster_level(s.pool) as i32,
                                character.ability_modifier(s.casting_ability),
                            )
                        })
                        .unwrap_or((0, 0));
                    Some((exprs, class_level as i32, caster_level, caster_modifier))
                })
                .collect();

            for (exprs, class_level, caster_level, caster_modifier) in feature_entries {
                let mut ctx = Context {
                    character,
                    class_level,
                    caster_level,
                    caster_modifier,
                };
                for expr in exprs {
                    if let Err(error) = expr.apply(&mut ctx) {
                        log::error!("Failed to apply assignment: {error:?}");
                    }
                }
            }
        });
    }

    /// Apply class level-up logic. Applies all unapplied levels from last
    /// applied+1 through the current class level. Handles saving throws,
    /// proficiencies, spell slots, class/species/background features, and HP.
    pub fn apply_class_level(&self, character: &mut Character, class_idx: usize, level: u32) {
        let Some(class_level) = character.identity.classes.get_mut(class_idx) else {
            return;
        };

        if class_level.applied_levels.contains(&level) {
            return;
        }

        let class_cache = self.class_cache.read_untracked();
        let Some(def) = class_cache.get(class_level.class.as_str()) else {
            return;
        };

        // Mark level as applied only after confirming the definition is loaded
        class_level.applied_levels.insert(level);
        let subclass = class_level.subclass.clone();
        let source = FeatureSource::Class(def.name.clone());

        // Record applied level and set hit die
        class_level.hit_die_sides = def.hit_die;

        // Apply class features: create SpellData, update slots, apply features
        let rules = def.levels.get(level as usize - 1);
        let subclass_rules = subclass
            .as_deref()
            .and_then(|sc| def.subclasses.get(sc))
            .and_then(|sc| sc.levels.get(&level));

        let features_guard = self.features_index.read_untracked();
        let Some(features_index) = features_guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| &idx.0)
        else {
            log::warn!("Features index not loaded yet, skipping level-up");
            class_level.applied_levels.remove(&level);
            return;
        };

        for feat_name in def.feature_names(subclass.as_deref()) {
            let Some(feat) = features_index.get(feat_name) else {
                log::warn!("Feature '{feat_name}' not found in index");
                continue;
            };
            let feat_name_string = feat_name.to_string();
            let is_new = rules.is_some_and(|r| r.features.contains(&feat_name_string))
                || subclass_rules.is_some_and(|r| r.features.contains(&feat_name_string));
            let already_has = character.features.iter().any(|f| f.name == feat_name);

            if is_new && already_has && !feat.stackable {
                continue;
            }

            if is_new || already_has {
                feat.apply(level, character, Some(&source));
            }
        }

        // Re-apply species and background features at new total level
        // (unlocks level-gated spells, e.g. Tiefling's Infernal Legacy)
        let total_level = character.level();

        let species_cache = self.species_cache.read_untracked();
        if let Some(species_def) = species_cache.get(character.identity.species.as_str()) {
            let source = FeatureSource::Species(character.identity.species.clone());
            for feat_name in &species_def.features {
                if let Some(feat) = features_index.get(feat_name.as_str()) {
                    feat.apply(total_level, character, Some(&source));
                }
            }
        }

        let bg_cache = self.background_cache.read_untracked();
        if let Some(bg_def) = bg_cache.get(character.identity.background.as_str()) {
            let source = FeatureSource::Background(character.identity.background.clone());
            for feat_name in &bg_def.features {
                if let Some(feat) = features_index.get(feat_name.as_str()) {
                    feat.apply(total_level, character, Some(&source));
                }
            }
        }

        // Recompute all derived stats (HP, AC, speed) + OnCompute assignments
        let old_hp_max = character.hp_max();
        self.compute(character);
        let hp_delta = character.hp_max().saturating_sub(old_hp_max);
        character.combat.hp_current += hp_delta;

        // Ensure XP meets the threshold for the new total level
        let xp_threshold = character.xp_threshold();
        if character.identity.experience_points < xp_threshold {
            character.identity.experience_points = xp_threshold;
        }
    }

    /// Like `apply_class_level`, but passes user-supplied args to features
    /// that need them. The `args_map` maps feature name → collected arg values.
    pub fn apply_class_level_with_args(
        &self,
        character: &mut Character,
        class_idx: usize,
        level: u32,
        args_map: &BTreeMap<String, Vec<i32>>,
    ) {
        let Some(class_level) = character.identity.classes.get_mut(class_idx) else {
            return;
        };

        if class_level.applied_levels.contains(&level) {
            return;
        }

        let class_cache = self.class_cache.read_untracked();
        let Some(def) = class_cache.get(class_level.class.as_str()) else {
            return;
        };

        class_level.applied_levels.insert(level);
        let subclass = class_level.subclass.clone();
        let source = FeatureSource::Class(def.name.clone());
        class_level.hit_die_sides = def.hit_die;

        let rules = def.levels.get(level as usize - 1);
        let subclass_rules = subclass
            .as_deref()
            .and_then(|sc| def.subclasses.get(sc))
            .and_then(|sc| sc.levels.get(&level));

        let features_guard = self.features_index.read_untracked();
        let Some(features_index) = features_guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| &idx.0)
        else {
            log::warn!("Features index not loaded yet, skipping level-up");
            character.identity.classes[class_idx]
                .applied_levels
                .remove(&level);
            return;
        };

        for feat_name in def.feature_names(subclass.as_deref()) {
            let Some(feat) = features_index.get(feat_name) else {
                log::warn!("Feature '{feat_name}' not found in index");
                continue;
            };
            let feat_name_string = feat_name.to_string();
            let is_new = rules.is_some_and(|r| r.features.contains(&feat_name_string))
                || subclass_rules.is_some_and(|r| r.features.contains(&feat_name_string));
            let already_has = character.features.iter().any(|f| f.name == feat_name);

            if is_new && already_has && !feat.stackable {
                continue;
            }

            if is_new || already_has {
                let args = args_map.get(feat_name).cloned();
                feat.apply_with_args(level, character, Some(&source), args);
            }
        }

        // Re-apply species and background features at new total level
        let total_level = character.level();

        let species_cache = self.species_cache.read_untracked();
        if let Some(species_def) = species_cache.get(character.identity.species.as_str()) {
            let source = FeatureSource::Species(character.identity.species.clone());
            for feat_name in &species_def.features {
                if let Some(feat) = features_index.get(feat_name.as_str()) {
                    feat.apply(total_level, character, Some(&source));
                }
            }
        }

        let bg_cache = self.background_cache.read_untracked();
        if let Some(bg_def) = bg_cache.get(character.identity.background.as_str()) {
            let source = FeatureSource::Background(character.identity.background.clone());
            for feat_name in &bg_def.features {
                if let Some(feat) = features_index.get(feat_name.as_str()) {
                    feat.apply(total_level, character, Some(&source));
                }
            }
        }

        let old_hp_max = character.hp_max();
        self.compute(character);
        let hp_delta = character.hp_max().saturating_sub(old_hp_max);
        character.combat.hp_current += hp_delta;

        let xp_threshold = character.xp_threshold();
        if character.identity.experience_points < xp_threshold {
            character.identity.experience_points = xp_threshold;
        }
    }

    /// Scan features that would be applied at a given class level and return
    /// those whose assignments require user-supplied ARG values.
    pub fn features_needing_args(
        &self,
        character: &Character,
        class_idx: usize,
        level: u32,
    ) -> Vec<PendingArgs> {
        let Some(class_level) = character.identity.classes.get(class_idx) else {
            return Vec::new();
        };

        let class_cache = self.class_cache.read_untracked();
        let Some(def) = class_cache.get(class_level.class.as_str()) else {
            return Vec::new();
        };

        let rules = def.levels.get(level as usize - 1);
        let subclass_rules = class_level
            .subclass
            .as_deref()
            .and_then(|sc| def.subclasses.get(sc))
            .and_then(|sc| sc.levels.get(&level));

        let mut result = Vec::new();
        self.with_features_index_untracked(|features_index| {
            // Only check features introduced at this specific level
            let level_features = rules
                .into_iter()
                .flat_map(|r| r.features.iter())
                .chain(subclass_rules.into_iter().flat_map(|r| r.features.iter()));

            for feat_name in level_features {
                let Some(feat) = features_index.get(feat_name.as_str()) else {
                    continue;
                };
                let already_has = character.features.iter().any(|f| f.name == *feat_name);

                if already_has && !feat.stackable {
                    continue;
                }

                let when = if already_has {
                    WhenCondition::OnLevelUp
                } else {
                    WhenCondition::OnFeatureAdd
                };

                if let Some(expr) = feat.args_expr(when) {
                    result.push(PendingArgs {
                        feature_name: feat_name.to_string(),
                        feature_label: feat.label().to_string(),
                        feature_description: feat.description.clone(),
                        expr: expr.clone(),
                    });
                }
            }
        });
        result
    }

    /// Check if a single feature (by name) needs ARG values for its apply.
    pub fn feature_needs_args(&self, character: &Character, name: &str) -> Option<PendingArgs> {
        self.with_features_index_untracked(|features_index| {
            let feat = features_index.get(name)?;
            let when = if character.feature_data.contains_key(name) {
                WhenCondition::OnLevelUp
            } else {
                WhenCondition::OnFeatureAdd
            };
            feat.args_expr(when).map(|expr| PendingArgs {
                feature_name: name.to_string(),
                feature_label: feat.label().to_string(),
                feature_description: feat.description.clone(),
                expr: expr.clone(),
            })
        })
    }

    /// Trigger spell list fetches for all feature data entries that reference
    /// external spell lists. Used by `fill_from_registry` before acquiring
    /// the spell list cache read guard.
    pub(super) fn trigger_spell_list_fetches(&self, character: &Character) {
        self.with_features_index_untracked(|features_index| {
            for key in character.feature_data.keys() {
                if let Some(feat_def) = find_feature(key, features_index)
                    && let Some(spells_def) = &feat_def.spells
                    && let SpellList::Ref { from } = &spells_def.list
                {
                    self.fetch_spell_list(from);
                }
            }
        });
    }

    /// Scan a list of feature names for those whose assignments require
    /// user-supplied ARG values.
    pub fn pending_args_for_features<'a>(
        &self,
        character: &Character,
        feature_names: impl Iterator<Item = &'a str>,
    ) -> Vec<PendingArgs> {
        let mut result = Vec::new();
        self.with_features_index_untracked(|features_index| {
            for feat_name in feature_names {
                let Some(feat) = features_index.get(feat_name) else {
                    continue;
                };
                let already_has = character.features.iter().any(|f| f.name == feat_name);
                if already_has && !feat.stackable {
                    continue;
                }
                let when = if already_has {
                    WhenCondition::OnLevelUp
                } else {
                    WhenCondition::OnFeatureAdd
                };
                if let Some(expr) = feat.args_expr(when) {
                    result.push(PendingArgs {
                        feature_name: feat_name.to_string(),
                        feature_label: feat.label().to_string(),
                        feature_description: feat.description.clone(),
                        expr: expr.clone(),
                    });
                }
            }
        });
        result
    }

    /// Apply a list of features by name with optional user-supplied args.
    fn apply_features(
        &self,
        character: &mut Character,
        feature_names: &[impl AsRef<str>],
        source: &FeatureSource,
        level: u32,
        args_map: Option<&BTreeMap<String, Vec<i32>>>,
    ) {
        self.with_features_index_untracked(|features_index| {
            for feat_name in feature_names {
                let feat_name = feat_name.as_ref();
                if let Some(feat) = features_index.get(feat_name) {
                    let args = args_map.and_then(|m| m.get(feat_name)).cloned();
                    feat.apply_with_args(level, character, Some(source), args);
                }
            }
        });
    }

    /// Apply species features from the global index.
    pub fn apply_species(
        &self,
        character: &mut Character,
        args_map: Option<&BTreeMap<String, Vec<i32>>>,
    ) {
        character.identity.species_applied = true;
        let total_level = character.level().max(1);
        let species_cache = self.species_cache.read_untracked();
        let Some(species_def) = species_cache.get(character.identity.species.as_str()) else {
            return;
        };
        let source = FeatureSource::Species(character.identity.species.clone());
        self.apply_features(
            character,
            &species_def.features,
            &source,
            total_level,
            args_map,
        );
        self.compute(character);
    }

    /// Apply background features from the global index.
    pub fn apply_background(
        &self,
        character: &mut Character,
        args_map: Option<&BTreeMap<String, Vec<i32>>>,
    ) {
        character.identity.background_applied = true;
        let total_level = character.level().max(1);
        let bg_cache = self.background_cache.read_untracked();
        let Some(bg_def) = bg_cache.get(character.identity.background.as_str()) else {
            return;
        };
        let source = FeatureSource::Background(character.identity.background.clone());
        self.apply_features(character, &bg_def.features, &source, total_level, args_map);
        self.compute(character);
    }
}
