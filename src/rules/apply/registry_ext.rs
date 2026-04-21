use leptos::prelude::*;

use crate::{
    model::{Character, Context, Expr, FeatureSource, FeatureValue},
    rules::{
        ReplaceWith, RulesRegistry, WhenCondition,
        apply::pending::PendingInputs,
        resolve::{find_feature, find_feature_with_class_level},
        spells::SpellList,
    },
};

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
        character.compute_armor_class();
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
            for (feat_name, entry) in character.features.data() {
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
                if let Some(entry) = character.features.get_mut(&feat_name)
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
                        .flat_map(|assigns| assigns.iter())
                        .filter(|assignment| assignment.when == when)
                        .collect();
                    if assignments.is_empty() {
                        return None;
                    }

                    // Group by scope target (None = own feature)
                    let mut scope_groups: Vec<(Option<&str>, Vec<Expr>)> = Vec::new();
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
                        .map(|spells_def| {
                            (
                                character.caster_level(spells_def.pool) as i32,
                                character.ability_modifier(spells_def.casting_ability),
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
                        .features
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
                    if let Some(feature_data) = ctx.character.features.get_mut(target) {
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
            PendingInputs::from_feature(
                name.to_string(),
                feat,
                source.cloned().unwrap_or_default(),
                when,
                Vec::new(),
                ReplaceWith::None,
            )
        })
    }

    /// Trigger spell list fetches for all feature data entries that reference
    /// external spell lists. Used by `fill_from_registry` before acquiring
    /// the spell list cache read guard.
    pub(in crate::rules) fn trigger_spell_list_fetches(&self, character: &Character) {
        self.with_features_index_untracked(|features_index| {
            for key in character.features.keys() {
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
