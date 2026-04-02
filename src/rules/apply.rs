use std::collections::BTreeMap;

use leptos::prelude::*;

use crate::{
    expr::Expr,
    model::{AssignInputs, Attribute, Character, Context, FeatureSource, FeatureValue},
    rules::{
        DefinitionStore, ReplaceWith, RulesRegistry, WhenCondition,
        background::BackgroundDefinition,
        class::ClassDefinition,
        feature::FeatureDefinition,
        resolve::{find_feature, find_feature_with_class_level},
        species::SpeciesDefinition,
        spells::SpellList,
    },
};

/// Bundled user inputs from the args/dice modal, keyed by feature name.
/// Each inner Vec has one entry per interactive assignment expression.
#[derive(Clone, Default)]
pub struct ApplyInputs {
    pub feature_inputs: BTreeMap<String, Vec<AssignInputs>>,
    /// Original feature name → replacement feature name.
    pub replacements: BTreeMap<String, String>,
}

impl ApplyInputs {
    pub fn get(&self, feature_name: &str) -> &[AssignInputs] {
        self.feature_inputs
            .get(feature_name)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

/// A feature whose assignment expressions require user interaction (ARG values
/// and/or dice rolls). Each expression in `exprs` gets its own independent
/// ARG context and dice pool.
#[derive(Clone, PartialEq)]
pub struct PendingInputs {
    pub feature_name: String,
    pub feature_label: String,
    pub feature_description: String,
    pub exprs: Vec<Expr<Attribute>>,
    pub replace_with: ReplaceWith,
    /// Source of the feature being added. Used by the replacement picker to
    /// determine if a stackable replacement is a new addition.
    pub source: FeatureSource,
}

impl PendingInputs {
    pub fn is_replaceable(&self) -> bool {
        !matches!(self.replace_with, ReplaceWith::None)
    }
}

/// A feature pending application. Owned and cheap — survives move closure
/// boundaries (modal callbacks). Produced by collect functions, consumed by
/// apply primitives.
#[derive(Clone)]
pub struct PendingFeature {
    pub name: String,
    pub source: FeatureSource,
    pub level: u32,
}

impl PendingFeature {
    /// Bridge to PendingInputs for the modal UI. Returns Some if this
    /// feature needs user interaction (ARG values, dice rolls, or is
    /// replaceable).
    pub fn pending_inputs(
        &self,
        feat_def: &FeatureDefinition,
        character: &Character,
    ) -> Option<PendingInputs> {
        let exprs = feat_def.interactive_exprs(WhenCondition::OnFeatureAdd, character);
        if exprs.is_empty() && !feat_def.is_replaceable() {
            return None;
        }
        Some(PendingInputs {
            feature_name: self.name.clone(),
            feature_label: feat_def.label().to_string(),
            feature_description: feat_def.description.clone(),
            exprs,
            replace_with: feat_def.replace_with,
            source: self.source.clone(),
        })
    }
}

// ── Collect functions ────────────────────────────────────────────────

/// Collect new features for a class level-up from class + subclass level rules.
/// Filters out already-applied features via dedup check.
pub fn collect_class_features<'a>(
    character: &'a Character,
    class_idx: usize,
    level: u32,
    class_def: &'a ClassDefinition,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
) -> impl Iterator<Item = PendingFeature> + 'a {
    let class_level = &character.identity.classes[class_idx];
    let source = FeatureSource::Class(class_def.name.clone(), level);

    let rules = class_def.levels.get(level as usize - 1);
    let subclass_rules = class_level
        .subclass
        .as_deref()
        .and_then(|sc| class_def.subclasses.get(sc))
        .and_then(|sc| sc.levels.get(&level));

    let filter_source = source.clone();
    rules
        .into_iter()
        .flat_map(|r| r.features.iter())
        .chain(subclass_rules.into_iter().flat_map(|r| r.features.iter()))
        .filter(move |feat_name| {
            features_index.get(feat_name.as_str()).is_none_or(|feat| {
                !character
                    .features
                    .contains(&feat.name, feat.stackable, &filter_source)
            })
        })
        .map(move |feat_name| PendingFeature {
            name: feat_name.clone(),
            source: source.clone(),
            level,
        })
}

/// Collect features from a species definition.
pub fn collect_species_features<'a>(
    character: &'a Character,
    species_def: &'a SpeciesDefinition,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
) -> impl Iterator<Item = PendingFeature> + 'a {
    let total_level = character.level().max(1);
    let source = FeatureSource::Species(character.identity.species.clone());
    let filter_source = source.clone();
    species_def
        .features
        .iter()
        .filter(move |feat_name| {
            features_index.get(feat_name.as_str()).is_none_or(|feat| {
                !character
                    .features
                    .contains(&feat.name, feat.stackable, &filter_source)
            })
        })
        .map(move |feat_name| PendingFeature {
            name: feat_name.clone(),
            source: source.clone(),
            level: total_level,
        })
}

/// Collect features from a background definition.
pub fn collect_background_features<'a>(
    character: &'a Character,
    bg_def: &'a BackgroundDefinition,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
) -> impl Iterator<Item = PendingFeature> + 'a {
    let total_level = character.level().max(1);
    let source = FeatureSource::Background(character.identity.background.clone());
    let filter_source = source.clone();
    bg_def
        .features
        .iter()
        .filter(move |feat_name| {
            features_index.get(feat_name.as_str()).is_none_or(|feat| {
                !character
                    .features
                    .contains(&feat.name, feat.stackable, &filter_source)
            })
        })
        .map(move |feat_name| PendingFeature {
            name: feat_name.clone(),
            source: source.clone(),
            level: total_level,
        })
}

/// Collect all unapplied features: species (if not applied), background
/// (if not applied), and class features for unapplied levels.
pub fn collect_pending_features(
    character: &Character,
    registry: &RulesRegistry,
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
) -> Vec<PendingFeature> {
    let species_cache = registry.species().cache().read_untracked();
    let bg_cache = registry.backgrounds().cache().read_untracked();
    let class_cache = registry.classes().cache().read_untracked();

    let species_iter = species_cache
        .get(character.identity.species.as_str())
        .filter(|_| !character.identity.species.is_empty() && !character.identity.species_applied)
        .into_iter()
        .flat_map(|species_def| collect_species_features(character, species_def, features_index));

    let bg_iter = bg_cache
        .get(character.identity.background.as_str())
        .filter(|_| {
            !character.identity.background.is_empty() && !character.identity.background_applied
        })
        .into_iter()
        .flat_map(|bg_def| collect_background_features(character, bg_def, features_index));

    let class_iter =
        character
            .identity
            .classes
            .iter()
            .enumerate()
            .flat_map(|(idx, class_level)| {
                let unapplied: Vec<u32> = (1..=class_level.level)
                    .filter(|lvl| !class_level.applied_levels.contains(lvl))
                    .collect();
                let class_def = class_cache.get(class_level.class.as_str());
                unapplied.into_iter().flat_map(move |lvl| {
                    class_def.into_iter().flat_map(move |def| {
                        collect_class_features(character, idx, lvl, def, features_index)
                    })
                })
            });

    species_iter.chain(bg_iter).chain(class_iter).collect()
}

// ── Apply primitives ─────────────────────────────────────────────────

/// Resolve replacement choices from modal inputs. For each pending feature
/// that has a replacement mapping, swap it with the replacement feature.
pub fn resolve_replacements(
    pending: &[PendingFeature],
    replacements: &BTreeMap<String, String>,
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
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

/// Add features to character.features and call feat.apply(OnFeatureAdd).
/// Looks up definitions from features_index by name.
pub fn apply_new_features(
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
    character: &mut Character,
    pending: &[PendingFeature],
    inputs: Option<&ApplyInputs>,
) {
    for pending_feature in pending {
        let Some(feat_def) = features_index.get(pending_feature.name.as_str()) else {
            log::warn!("Feature '{}' not found in index", pending_feature.name);
            continue;
        };
        if character
            .features
            .contains(&feat_def.name, feat_def.stackable, &pending_feature.source)
        {
            continue;
        }
        let feature_inputs = inputs
            .map(|i| i.get(&pending_feature.name))
            .unwrap_or_default();
        character.features.add(
            &pending_feature.name,
            feat_def.label.clone(),
            feat_def.description.clone(),
            pending_feature.source.clone(),
            feature_inputs.to_vec(),
        );
        feat_def.apply(
            pending_feature.level,
            character,
            WhenCondition::OnFeatureAdd,
            feature_inputs,
        );
    }
}

/// Re-apply OnLevelUp for all currently applied features at their appropriate
/// level. Class features use their class's current level;
/// species/background/user features use total character level.
pub fn reapply_existing(
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
    character: &mut Character,
) {
    let applied: Vec<(String, FeatureSource)> = character
        .features
        .iter()
        .filter(|f| f.applied)
        .map(|f| (f.name.clone(), f.source.clone()))
        .collect();
    for (feat_name, source) in &applied {
        if let Some(feat_def) = features_index.get(feat_name.as_str()) {
            let level = character.effective_level_for(source);
            feat_def.apply(level, character, WhenCondition::OnLevelUp, &[]);
        }
    }
}

/// Replay: reset derived state and re-apply all features from stored data.
/// `pending` should be sorted by `added_at_level`. `inputs` supplies
/// supplemental ARG values for features that lack stored inputs.
pub fn replay(
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
    character: &mut Character,
    pending: &[PendingFeature],
    inputs: &ApplyInputs,
) {
    character.reset_computed();

    // Phase 1: OnFeatureAdd at added_at_level.
    // Collect stored inputs upfront to avoid borrow conflict with
    // def.apply() which takes &mut Character.
    let stored_inputs: Vec<_> = pending
        .iter()
        .map(|pending_feature| {
            character
                .features
                .get_inputs(&pending_feature.name)
                .to_vec()
        })
        .collect();
    for (pending_feature, stored) in pending.iter().zip(&stored_inputs) {
        let Some(feat_def) = features_index.get(pending_feature.name.as_str()) else {
            continue;
        };
        let feature_inputs = if stored.is_empty() {
            inputs.get(&pending_feature.name)
        } else {
            stored.as_slice()
        };
        feat_def.apply(
            pending_feature.level,
            character,
            WhenCondition::OnFeatureAdd,
            feature_inputs,
        );
    }

    // Persist supplemental inputs back to Feature entries
    for feature in character.features.iter_mut() {
        if feature.inputs.is_empty() {
            let supp = inputs.get(&feature.name);
            if !supp.is_empty() {
                feature.inputs = supp.to_vec();
            }
        }
    }

    // Phase 2: OnLevelUp through intermediate levels
    for pending_feature in pending {
        let Some(feat_def) = features_index.get(pending_feature.name.as_str()) else {
            continue;
        };
        let effective = character.effective_level_for(&pending_feature.source);
        for level in (pending_feature.level + 1)..=effective {
            feat_def.apply(level, character, WhenCondition::OnLevelUp, &[]);
        }
    }
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

            // Collect per-feature info with scope-grouped assignments.
            // Each entry: (scope_groups, class_level, caster_level, caster_modifier)
            // where scope_groups: Vec<(scope_target, Vec<Expr>)>
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
                    let assignments: Vec<_> = feat_def
                        .assign
                        .iter()
                        .flat_map(|a| a.iter())
                        .filter(|a| a.when == when)
                        .collect();
                    if assignments.is_empty() {
                        return None;
                    }

                    // Group by scope target (None = own feature)
                    let mut scope_groups: Vec<(Option<&str>, Vec<Expr<Attribute>>)> = Vec::new();
                    for assignment in &assignments {
                        let scope = assignment.scope.as_deref();
                        if let Some(group) = scope_groups.iter_mut().find(|(s, _)| *s == scope) {
                            group.1.push(assignment.expr.clone());
                        } else {
                            scope_groups.push((scope, vec![assignment.expr.clone()]));
                        }
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
                    Some((
                        feat.name.clone(),
                        scope_groups
                            .into_iter()
                            .map(|(scope, exprs)| (scope.map(String::from), exprs))
                            .collect::<Vec<_>>(),
                        class_level as i32,
                        caster_level,
                        caster_modifier,
                    ))
                })
                .collect();

            for (feat_name, scope_groups, class_level, caster_level, caster_modifier) in
                feature_entries
            {
                for (scope, exprs) in scope_groups {
                    let target = scope.as_deref().unwrap_or(&feat_name);
                    let points = character
                        .feature_data
                        .get(target)
                        .map(Context::extract_points)
                        .unwrap_or_default();

                    let mut ctx = Context {
                        character,
                        class_level,
                        caster_level,
                        caster_modifier,
                        points,
                    };
                    for expr in &exprs {
                        if let Err(error) = expr.apply(&mut ctx) {
                            log::error!("Failed to apply assignment: {error:?}");
                        }
                    }

                    // Write back modified points
                    if let Some(feature_data) = ctx.character.feature_data.get_mut(target) {
                        Context::writeback_points(feature_data, &ctx.points);
                    }
                }
            }
        });
    }

    /// Check if a single feature (by name) needs user interaction for its
    /// apply (ARG values or dice rolls). When `source` is provided (e.g. for
    /// replacement features), uses source-aware dedup for stackable features.
    pub fn feature_needs_args(
        &self,
        character: &Character,
        name: &str,
        source: Option<&FeatureSource>,
    ) -> Option<PendingInputs> {
        self.with_features_index_untracked(|features_index| {
            let feat = features_index.get(name)?;
            let when = match source {
                Some(source) if feat.stackable => {
                    if character
                        .features
                        .contains(&feat.name, feat.stackable, source)
                    {
                        WhenCondition::OnLevelUp
                    } else {
                        WhenCondition::OnFeatureAdd
                    }
                }
                _ => {
                    if character.features.is_pending(name) {
                        WhenCondition::OnFeatureAdd
                    } else {
                        WhenCondition::OnLevelUp
                    }
                }
            };
            let exprs = feat.interactive_exprs(when, character);
            (!exprs.is_empty()).then_some(PendingInputs {
                feature_name: name.to_string(),
                feature_label: feat.label().to_string(),
                feature_description: feat.description.clone(),
                exprs,
                replace_with: ReplaceWith::None,
                source: source.cloned().unwrap_or_default(),
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
}
