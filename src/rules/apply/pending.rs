use std::collections::BTreeMap;

use crate::{
    model::{AssignInputs, Character, Expr, FeatureSource},
    rules::{ReplaceWith, WhenCondition, feature::FeatureDefinition},
};

/// Key for per-feature-instance inputs. Stackable features appear with the
/// same `name` but different `source`, so both identify the instance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FeatureKey {
    pub name: String,
    pub source: FeatureSource,
}

impl FeatureKey {
    pub fn new(name: impl Into<String>, source: FeatureSource) -> Self {
        Self {
            name: name.into(),
            source,
        }
    }

    pub fn from_pending(pending: &PendingFeature) -> Self {
        Self::new(&*pending.name, pending.source.clone())
    }
}

/// Bundled user inputs from the args/dice modal, keyed by `FeatureKey`.
/// Using `(name, source)` as the key lets stackable features with multiple
/// instances (e.g. ASI at Monk L4 and Monk L8) carry distinct inputs —
/// a name-only map would collapse them into one. Each inner `Vec` has one
/// entry per interactive assignment expression of the feature instance.
#[derive(Clone, Default)]
pub struct ApplyInputs {
    pub feature_inputs: BTreeMap<FeatureKey, Vec<AssignInputs>>,
    /// Original feature name → replacement feature name.
    pub replacements: BTreeMap<String, String>,
}

impl ApplyInputs {
    pub fn get(&self, feature_name: &str, source: &FeatureSource) -> &[AssignInputs] {
        // Lookup allocates once to build the key — acceptable: `get` is
        // called only a handful of times per apply flow, not in any hot
        // path. In exchange we get O(log n) BTreeMap lookup and a clean
        // owned-key storage model.
        let key = FeatureKey::new(feature_name, source.clone());
        self.feature_inputs.get(&key).map_or(&[], Vec::as_slice)
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
    pub exprs: Vec<Expr>,
    /// Existing stored inputs aligned with `exprs` (by index). Empty when the
    /// feature is being applied for the first time. Used by the modal to
    /// pre-fill ARG and dice signals so re-apply behaves as edit.
    pub prefill: Vec<AssignInputs>,
    pub replace_with: ReplaceWith,
    /// Pre-chosen replacement feature name (e.g. from AI generation). When set,
    /// `ReplacementPicker` initializes `replacement_choice` with this value and
    /// shows the replacement UI expanded. User can still override.
    pub prefilled_replacement: Option<String>,
    /// Pre-filled ARG values for the replacement feature's expressions. Used
    /// only when `prefilled_replacement` is `Some`. Same value broadcast to
    /// every expr of the replacement (matches non-replacement `prefill`
    /// semantics — one AI-supplied `args` vec applies to all interactive
    /// exprs of the feature).
    pub replacement_prefill: Option<AssignInputs>,
    /// Source of the feature being added. Used by the replacement picker to
    /// determine if a stackable replacement is a new addition.
    pub source: FeatureSource,
    /// When `true`, the input is fully determined and the modal hides the
    /// form; the cascade still applies the feat so downstream snapshots
    /// see its effect in pipeline order. Two sources set this:
    ///
    /// - Non-interactive feats (no `@ARG`/dice anywhere) — nothing to pick, but
    ///   we want the effect (e.g. `SKILL.PERC.PROF = 1`) in the cascade
    ///   baseline.
    /// - Effective-stored interactive feats — args already decided in a
    ///   previous apply; re-asking would be noise.
    pub hidden: bool,
}

impl PendingInputs {
    pub fn is_replaceable(&self) -> bool {
        !matches!(self.replace_with, ReplaceWith::None)
    }

    pub fn is_replace_only(&self) -> bool {
        self.is_replaceable() && self.exprs.is_empty()
    }

    /// Build a `PendingInputs` from a feature definition. Returns `None` if
    /// the feature has no interactive exprs under `when` and is not
    /// replaceable (by the supplied `replace_with`).
    pub fn from_feature(
        name: String,
        feat_def: &FeatureDefinition,
        source: FeatureSource,
        when: WhenCondition,
        prefill: Vec<AssignInputs>,
        replace_with: ReplaceWith,
    ) -> Option<Self> {
        let exprs = feat_def.interactive_exprs(when);
        let is_replaceable = !matches!(replace_with, ReplaceWith::None);
        if exprs.is_empty() && !is_replaceable {
            return None;
        }
        Some(Self {
            feature_name: name,
            feature_label: feat_def.label().to_string(),
            feature_description: feat_def.description.clone(),
            exprs,
            prefill,
            replace_with,
            prefilled_replacement: None,
            replacement_prefill: None,
            source,
            hidden: false,
        })
    }

    /// Build a hidden `PendingInputs` for a non-interactive feat that still
    /// needs to participate in the cascade (e.g. feats with hardcoded
    /// `SKILL.X.PROF = 1`). No form is rendered; the cascade's per-pending
    /// Effect applies the feat with empty inputs so downstream snapshots
    /// include its derived-state effect.
    pub fn hidden_for_cascade(
        name: String,
        feat_def: &FeatureDefinition,
        source: FeatureSource,
    ) -> Self {
        Self {
            feature_name: name,
            feature_label: feat_def.label().to_string(),
            feature_description: feat_def.description.clone(),
            exprs: Vec::new(),
            prefill: Vec::new(),
            replace_with: feat_def.replace_with,
            prefilled_replacement: None,
            replacement_prefill: None,
            source,
            hidden: true,
        }
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
        let prefill = character
            .features
            .iter()
            .find(|feature| {
                feature.name == self.name
                    && feature.applied
                    && (!feat_def.stackable || feature.source == self.source)
            })
            .map(|feature| feature.inputs.clone())
            .unwrap_or_default();
        PendingInputs::from_feature(
            self.name.clone(),
            feat_def,
            self.source.clone(),
            WhenCondition::OnFeatureAdd,
            prefill,
            feat_def.replace_with,
        )
    }
}
