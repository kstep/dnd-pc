use crate::{
    model::{Armor, ArmorType, Attribute, Character, FeatureSource},
    rules::{
        ReplaceWith, RulesRegistry, WhenCondition,
        apply::{compute, context::ApplyContext, pending::PendingInputs},
        feature::{Assignment, FeatureDefinition},
        spells::SpellsList,
    },
};

impl RulesRegistry {
    pub fn long_rest(&self, character: &mut Character) {
        character.long_rest();
        self.with_features_index_untracked(|feat_index| {
            crate::rules::apply::assign(character, feat_index, WhenCondition::OnLongRest);
        });
        crate::rules::apply::assign_items(character, WhenCondition::OnLongRest);
    }

    pub fn short_rest(&self, character: &mut Character) {
        character.short_rest();
        self.with_features_index_untracked(|feat_index| {
            crate::rules::apply::assign(character, feat_index, WhenCondition::OnShortRest);
        });
        crate::rules::apply::assign_items(character, WhenCondition::OnShortRest);
    }

    /// Apply a single feature's lifecycle pass: spells skeleton init,
    /// assignments matching `when`, and natural-armor auto-detection. Choice
    /// fields are lazy-created via `CHOICE.<name>.COUNT` assigns. The feature
    /// must already exist at `character.features.list[feature_index]` —
    /// callers push it before invoking.
    pub fn apply(&self, character: &mut Character, feature_index: usize, when: WhenCondition) {
        let feature_name = character.features.list[feature_index].name.clone();
        self.with_features_index_untracked(|feat_index| {
            let Some(feat_def) = feat_index.get(feature_name.as_str()) else {
                return;
            };
            apply_feature(feat_def, character, feature_index, when);
        });
    }

    /// Thin wrapper over the free `compute(character, feat_index)` for
    /// UI-side callers that hold a registry but no direct index handle.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.compute", skip_all)
    )]
    pub fn compute(&self, character: &mut Character) {
        self.with_features_index_untracked(|feat_index| {
            compute(character, feat_index);
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

    /// Trigger per-class name list fetches for any feature whose spells block
    /// is `Ref { from }`. Used by `fill_from_registry` before reading the
    /// names cache.
    pub(in crate::rules) fn trigger_spell_list_fetches(&self, character: &Character) {
        self.with_features_index_untracked(|features_index| {
            for key in character.features.keys() {
                if let Some(feat_def) = features_index.get(key.as_str())
                    && let Some(spells_def) = &feat_def.spells
                    && let SpellsList::Ref { from } = &spells_def.list
                {
                    self.fetch_spell_list_untracked(from);
                }
            }
        });
    }
}

/// Free function for callers (solver, rebuild dry-runs, registry methods)
/// that already hold a `&FeatureDefinition`. Mirrors the registry's
/// `apply` lifecycle but skips the index lookup so dry-run paths can apply
/// against cloned characters without going through the registry.
pub fn apply_feature(
    feat_def: &FeatureDefinition,
    character: &mut Character,
    feature_index: usize,
    when: WhenCondition,
) {
    // 1. Ensure feature data entry exists so downstream code (UI, labels)
    // can address it by name even if no spells/assigns populate it.
    let feature_data = character
        .features
        .entry(feat_def.name.to_string())
        .or_default();

    // 2. Spells skeleton (idempotent — `get_or_insert`).
    if feat_def.spells.is_some() {
        feature_data.spells.get_or_insert_with(Default::default);
    }

    // 3. Assignments via ApplyContext.
    if let Some(assignments) = feat_def.assign.as_ref() {
        ApplyContext::new(character, feature_index).run_assignments(assignments, when);
    }

    // 4. Natural armor auto-detection — exactly one OnCompute AC assignment
    // surfaces an Armor entry so the equipment list shows it.
    if let Some(ac_assign) = single_ac_assignment(feat_def) {
        let already_exists = character.equipment.armors.iter().any(|armor| {
            armor.armor_type == ArmorType::Natural && armor.name.as_str() == &*feat_def.name
        });
        if !already_exists {
            character.equipment.armors.push(Armor {
                name: feat_def.name.to_string(),
                armor_type: ArmorType::Natural,
                ac_expr: Some(ac_assign.expr.clone()),
                ..Default::default()
            });
        }
    }
}

/// Returns the single `OnCompute` assignment that writes to `AC`, if
/// exactly one such assignment exists.
fn single_ac_assignment(feat_def: &FeatureDefinition) -> Option<&Assignment> {
    let assignments = feat_def.assign.as_ref()?;
    let mut ac_iter = assignments.iter().filter(|a| {
        a.when == WhenCondition::OnCompute && a.expr.assigns_to(|v| matches!(v, Attribute::Ac))
    });
    let first = ac_iter.next()?;
    if ac_iter.next().is_some() {
        return None;
    }
    Some(first)
}
