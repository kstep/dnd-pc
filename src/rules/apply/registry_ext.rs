use crate::{
    model::{Character, FeatureSource},
    rules::{
        ReplaceWith, RulesRegistry, WhenCondition,
        apply::{compute, pending::PendingInputs},
        resolve::find_feature,
        spells::SpellList,
    },
};

impl RulesRegistry {
    pub fn long_rest(&self, character: &mut Character) {
        character.long_rest();
        self.with_features_index_untracked(|fi| {
            crate::rules::apply::assign(character, fi, WhenCondition::OnLongRest);
        });
    }

    pub fn short_rest(&self, character: &mut Character) {
        character.short_rest();
        self.with_features_index_untracked(|fi| {
            crate::rules::apply::assign(character, fi, WhenCondition::OnShortRest);
        });
    }

    /// Thin wrapper over the free `compute(character, fi)` for UI-side
    /// callers that hold a registry but no direct features-index handle.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.compute", skip_all)
    )]
    pub fn compute(&self, character: &mut Character) {
        self.with_features_index_untracked(|fi| compute(character, fi));
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
