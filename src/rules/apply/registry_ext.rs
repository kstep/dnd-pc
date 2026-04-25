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

    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.compute", skip_all)
    )]
    pub fn compute(&self, character: &mut Character) {
        character.compute();
        self.refresh_spell_structure(character);
        self.assign(character, WhenCondition::OnCompute);
        character.compute_armor_class();
        self.recompute_dynamic_fields(character);
    }

    /// Bootstrap per-feature `SpellData` for all features carrying a
    /// `SpellsDefinition`: ensure the entry exists, import sticky spells
    /// from inline lists, and refresh `free_uses.max` on prepared spells.
    ///
    /// Numeric scaling — slot totals, prepared / known counts, cantrip
    /// counts — is driven by `OnFeatureAdd` and `OnCompute` `assign`
    /// expressions on the feature itself (see `Slot`, `SlotPool`,
    /// `CasterAbility`, `CasterCoef`, `SpellCantrips`, `SpellReady`,
    /// `SpellKnown` resolvers in `src/model/character.rs`). This function
    /// only handles structural bootstrap; calling it before the
    /// `OnCompute` pass guarantees `Context::pool` resolves through the
    /// existing SpellData when scaling assigns run.
    fn refresh_spell_structure(&self, character: &mut Character) {
        self.with_features_index_untracked(|features_index| {
            // Snapshot before mutating: `spells_def.apply` takes
            // `&mut Character` and `effective_level_for` / `free_uses_max`
            // hold immutable borrows during iteration.
            let updates: Vec<(String, u32, u32)> = character
                .features
                .iter()
                .filter_map(|feature| {
                    let feat_def = features_index.get(feature.name.as_str())?;
                    feat_def.spells.as_ref()?;
                    let level = character.effective_level_for(&feature.source);
                    let free_uses_max = feat_def.free_uses_max(level, character);
                    Some((feature.name.clone(), level, free_uses_max))
                })
                .collect();
            for (feat_name, level, free_uses_max) in updates {
                if let Some(feat_def) = features_index.get(feat_name.as_str())
                    && let Some(spells_def) = &feat_def.spells
                {
                    spells_def.apply(level, character, &feat_name, free_uses_max);
                }
            }
        });
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

                    Some((
                        feat.name.clone(),
                        scope_groups
                            .into_iter()
                            .map(|(scope, exprs)| (scope.map(String::from), exprs))
                            .collect::<Vec<_>>(),
                        class_level as i32,
                    ))
                })
                .collect();

            for (feat_name, scope_groups, class_level) in feature_entries {
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
                        feature: Some(target.to_string()),
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
        name: &str,
        source: Option<&FeatureSource>,
    ) -> Option<PendingInputs> {
        self.with_features_index_untracked(|features_index| {
            let feat = features_index.get(name)?;
            PendingInputs::from_feature(
                name.to_string(),
                feat,
                source.cloned().unwrap_or_default(),
                WhenCondition::OnFeatureAdd,
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
