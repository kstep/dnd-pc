use crate::{
    model::{Character, CharacterCore, FeatureSource},
    rules::{
        ReplaceWith, RulesRegistry, WhenCondition,
        apply::{
            IdentityChange, assign, assign_items, compute, compute_core, context::ApplyContext,
            pending::PendingInputs,
        },
        feature::FeatureDefinition,
        spells::SpellsList,
    },
};

impl RulesRegistry {
    pub fn long_rest(&self, character: &mut Character) {
        character.long_rest();
        self.with_features_index_untracked(|feat_index| {
            assign(character, feat_index, WhenCondition::OnLongRest);
        });
        assign_items(character, WhenCondition::OnLongRest);
    }

    pub fn short_rest(&self, character: &mut Character) {
        character.short_rest();
        self.with_features_index_untracked(|feat_index| {
            assign(character, feat_index, WhenCondition::OnShortRest);
        });
        assign_items(character, WhenCondition::OnShortRest);
    }

    /// Apply a single feature's lifecycle pass: spells skeleton init,
    /// assignments matching `when`, and natural-armor auto-detection. Choice
    /// fields are lazy-created via `CHOICE.<name>.COUNT` assigns. The feature
    /// must already exist at `character.features.list[feature_pos]` —
    /// callers push it before invoking.
    pub fn apply(&self, character: &mut Character, feature_pos: usize, when: WhenCondition) {
        let feature_name = character.features.list[feature_pos].name.clone();
        self.with_features_index_untracked(|feat_index| {
            let Some(feat_def) = feat_index.get(feature_name.as_str()) else {
                return;
            };
            apply_feature(feat_def, character, feature_pos, when);
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

    /// Core-only `compute` for speculative cascade previews. Skips equipment
    /// AC evaluation — args-modal panels don't read AC.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.compute_core", skip_all)
    )]
    pub fn compute_core(&self, character: &mut CharacterCore) {
        self.with_features_index_untracked(|feat_index| {
            compute_core(character, feat_index);
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
    pub fn trigger_spell_list_fetches(&self, character: &CharacterCore) {
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
    character: &mut CharacterCore,
    feature_pos: usize,
    when: WhenCondition,
) -> Vec<IdentityChange> {
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

    // 3. Assignments via ApplyContext. Drained identity events flow back to
    // the caller for `applied`-flag updates and feature-collection follow-ups.
    if let Some(assignments) = feat_def.assign.as_ref() {
        let mut ctx = ApplyContext::new(character, feature_pos);
        ctx.run_assignments(assignments, when);
        ctx.take_identity_changes()
    } else {
        Vec::new()
    }
}
