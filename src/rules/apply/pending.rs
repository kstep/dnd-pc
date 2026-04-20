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
    /// Source of the feature being added. Used by the replacement picker to
    /// determine if a stackable replacement is a new addition.
    pub source: FeatureSource,
}

impl PendingInputs {
    pub fn is_replaceable(&self) -> bool {
        !matches!(self.replace_with, ReplaceWith::None)
    }

    pub fn is_replace_only(&self) -> bool {
        self.is_replaceable() && self.exprs.is_empty()
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
        Some(PendingInputs {
            feature_name: self.name.clone(),
            feature_label: feat_def.label().to_string(),
            feature_description: feat_def.description.clone(),
            exprs,
            prefill,
            replace_with: feat_def.replace_with,
            source: self.source.clone(),
        })
    }
}
