use crate::{
    model::{Character, FeatureSource},
    rules::{
        ReplaceWith, RulesRegistry, WhenCondition,
        apply::{compute, pending::PendingInputs},
        spells::SpellsList,
    },
};

impl RulesRegistry {
    pub fn long_rest(&self, character: &mut Character) {
        character.long_rest();
        self.with_features_index_untracked(|feat_index| {
            crate::rules::apply::assign(character, feat_index, WhenCondition::OnLongRest);
        });
    }

    pub fn short_rest(&self, character: &mut Character) {
        character.short_rest();
        self.with_features_index_untracked(|feat_index| {
            crate::rules::apply::assign(character, feat_index, WhenCondition::OnShortRest);
        });
    }

    /// Thin wrapper over the free `compute(character, feat_index, spell_index)`
    /// for UI-side callers that hold a registry but no direct index
    /// handles.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.compute", skip_all)
    )]
    pub fn compute(&self, character: &mut Character) {
        self.with_apply_indexes(|feat_index, spell_index| {
            compute(character, feat_index, spell_index);
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
