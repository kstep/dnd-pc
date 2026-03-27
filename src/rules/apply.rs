use std::collections::{BTreeMap, VecDeque};

use leptos::prelude::*;

use super::{
    WhenCondition,
    resolve::{find_feature, find_feature_with_class_level},
    spells::SpellList,
};
use crate::{
    expr::Expr,
    model::{Attribute, Character, Context, Feature, FeatureSource, FeatureValue},
    rules::RulesRegistry,
};

/// A feature whose assignment expressions require user-supplied ARG values.
/// Each expression in `exprs` gets its own independent ARG context.
#[derive(Clone, PartialEq)]
pub struct PendingArgs {
    pub feature_name: String,
    pub feature_label: String,
    pub feature_description: String,
    pub exprs: Vec<Expr<Attribute>>,
    pub replaceable: bool,
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

    /// Reset character to base state and re-apply all features level by level,
    /// reusing previously stored ARG values.
    // TODO: replay does not handle feature replacements
    pub fn replay(&self, character: &mut Character) {
        // Extract stored args before reset
        let mut stored_args: BTreeMap<String, VecDeque<Vec<Vec<i32>>>> = BTreeMap::new();
        for (name, data) in &character.feature_data {
            if !data.args.is_empty() {
                let args: VecDeque<Vec<Vec<i32>>> = data
                    .args
                    .iter()
                    .map(|assign_args| vec![assign_args.values.clone()])
                    .collect();
                stored_args.insert(name.clone(), args);
            }
        }

        character.reset_for_replay();

        // Re-apply level by level for each class
        let class_count = character.identity.classes.len();
        for class_idx in 0..class_count {
            let max_level = character.identity.classes[class_idx].level;
            for level in 1..=max_level {
                let args_map =
                    self.build_replay_args(character, class_idx, level, &mut stored_args);
                let args_ref = if args_map.is_empty() {
                    None
                } else {
                    Some(&args_map)
                };
                self.apply_class_level(character, class_idx, level, args_ref, None);
            }
        }
    }

    /// Build args_map for a single level's features by popping stored args.
    fn build_replay_args(
        &self,
        character: &Character,
        class_idx: usize,
        level: u32,
        stored_args: &mut BTreeMap<String, VecDeque<Vec<Vec<i32>>>>,
    ) -> BTreeMap<String, Vec<Vec<i32>>> {
        let mut args_map = BTreeMap::new();

        let Some(class_level) = character.identity.classes.get(class_idx) else {
            return args_map;
        };
        let class_cache = self.class_cache.read_untracked();
        let Some(def) = class_cache.get(class_level.class.as_str()) else {
            return args_map;
        };

        let rules = def.levels.get(level as usize - 1);
        let subclass_rules = class_level
            .subclass
            .as_deref()
            .and_then(|sc| def.subclasses.get(sc))
            .and_then(|sc| sc.levels.get(&level));

        let level_features = rules
            .into_iter()
            .flat_map(|r| r.features.iter())
            .chain(subclass_rules.into_iter().flat_map(|r| r.features.iter()));

        for feat_name in level_features {
            if let Some(queue) = stored_args.get_mut(feat_name.as_str())
                && let Some(args) = queue.pop_front()
            {
                args_map.insert(feat_name.to_string(), args);
            }
        }

        args_map
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

    /// Apply class level-up logic. Handles saving throws, proficiencies,
    /// spell slots, class/species/background features, and HP. Optional
    /// `args_map` passes user-supplied ARG values to features that need them.
    /// `replacements` maps original feature name → replacement feature name;
    /// replaced features are skipped and the replacement feat is applied
    /// with the same source and level.
    pub fn apply_class_level(
        &self,
        character: &mut Character,
        class_idx: usize,
        level: u32,
        args_map: Option<&BTreeMap<String, Vec<Vec<i32>>>>,
        replacements: Option<&BTreeMap<String, String>>,
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

        // Re-apply already applied features at new level (OnLevelUp)
        let applied: Vec<String> = character
            .features
            .iter()
            .filter(|f| f.applied)
            .map(|f| f.name.clone())
            .collect();
        for feat_name in &applied {
            if let Some(feat) = features_index.get(feat_name.as_str()) {
                feat.apply_with_args(level, character, Some(&source), None);
            }
        }

        // New features at this level (OnFeatureAdd)
        let level_features = rules
            .into_iter()
            .flat_map(|r| r.features.iter())
            .chain(subclass_rules.into_iter().flat_map(|r| r.features.iter()));

        for feat_name in level_features {
            // If this feature was replaced, apply the replacement instead
            if let Some(replacement_name) = replacements.and_then(|r| r.get(feat_name.as_str())) {
                if let Some(replacement_feat) = features_index.get(replacement_name.as_str()) {
                    let args = args_map
                        .and_then(|m| m.get(replacement_name.as_str()))
                        .cloned();
                    replacement_feat.apply_with_args(level, character, Some(&source), args);
                } else {
                    log::warn!("Replacement feature '{replacement_name}' not found in index");
                }
                continue;
            }

            let Some(feat) = features_index.get(feat_name.as_str()) else {
                log::warn!("Feature '{feat_name}' not found in index");
                continue;
            };
            let already_has = character
                .features
                .iter()
                .any(|f| f.name == *feat_name && f.applied);
            if already_has && !feat.stackable {
                continue;
            }
            // For stackable features being added again, pre-push an unapplied
            // entry so mark_feature_applied() finds it.
            if already_has && feat.stackable && !character.is_feature_pending(feat_name) {
                character.features.push(Feature {
                    name: feat_name.to_string(),
                    label: feat.label.clone(),
                    description: feat.description.clone(),
                    applied: false,
                });
            }
            let args = args_map.and_then(|m| m.get(feat_name.as_str())).cloned();
            feat.apply_with_args(level, character, Some(&source), args);
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

    /// Scan features at a given class level and return those that need
    /// user interaction before applying: features with ARG expressions
    /// and/or features marked `replaceable`.
    pub fn features_pending_input(
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
            // New features at this level → OnFeatureAdd
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
                let exprs: Vec<_> = feat
                    .args_exprs(WhenCondition::OnFeatureAdd)
                    .cloned()
                    .collect();
                if !exprs.is_empty() || feat.replaceable {
                    result.push(PendingArgs {
                        feature_name: feat_name.to_string(),
                        feature_label: feat.label().to_string(),
                        feature_description: feat.description.clone(),
                        exprs,
                        replaceable: feat.replaceable,
                    });
                }
            }

            // Already applied features → OnLevelUp
            for feature in &character.features {
                if !feature.applied {
                    continue;
                }
                let Some(feat) = features_index.get(feature.name.as_str()) else {
                    continue;
                };
                // Skip if already collected as OnFeatureAdd
                if result.iter().any(|r| r.feature_name == feature.name) {
                    continue;
                }
                let exprs: Vec<_> = feat.args_exprs(WhenCondition::OnLevelUp).cloned().collect();
                if !exprs.is_empty() {
                    result.push(PendingArgs {
                        feature_name: feature.name.clone(),
                        feature_label: feat.label().to_string(),
                        feature_description: feat.description.clone(),
                        exprs,
                        replaceable: false,
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
            let when = if character.is_feature_pending(name) {
                WhenCondition::OnFeatureAdd
            } else {
                WhenCondition::OnLevelUp
            };
            let exprs: Vec<_> = feat.args_exprs(when).cloned().collect();
            (!exprs.is_empty()).then_some(PendingArgs {
                feature_name: name.to_string(),
                feature_label: feat.label().to_string(),
                feature_description: feat.description.clone(),
                exprs,
                replaceable: false,
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
                let when = if character.is_feature_pending(feat_name) {
                    WhenCondition::OnFeatureAdd
                } else {
                    WhenCondition::OnLevelUp
                };
                let exprs: Vec<_> = feat.args_exprs(when).cloned().collect();
                if !exprs.is_empty() {
                    result.push(PendingArgs {
                        feature_name: feat_name.to_string(),
                        feature_label: feat.label().to_string(),
                        feature_description: feat.description.clone(),
                        exprs,
                        replaceable: false,
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
        args_map: Option<&BTreeMap<String, Vec<Vec<i32>>>>,
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
        args_map: Option<&BTreeMap<String, Vec<Vec<i32>>>>,
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
        args_map: Option<&BTreeMap<String, Vec<Vec<i32>>>>,
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
