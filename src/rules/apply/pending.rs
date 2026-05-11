use std::collections::BTreeMap;

use crate::{
    model::{AssignInputs, CharacterCore, Expr, FeatureSource},
    rules::{ReplaceWith, WhenCondition, feature::FeatureDefinition},
};

/// Names of the identity-slot picker placeholder features defined in
/// `public/data/features.json`. Each carries a
/// `replace_with: Category(System(_))` so the args modal can swap it for the
/// user's pick during cascade.
pub const PICK_SPECIES: &str = "Generation: Species";
pub const PICK_BACKGROUND: &str = "Generation: Background";
pub const PICK_CLASS: &str = "Class Level";
pub const PICK_SUBCLASS: &str = "Subclass";

/// Speculative-cascade recompute closure: given a tentative character, return
/// the pending list the modal should display. `None` disables speculation.
pub type RecomputePending = Box<dyn Fn(&CharacterCore) -> Vec<PendingInputs> + Send + Sync>;

/// Key for per-feature-instance inputs. Stackable features appear with the
/// same `name` but different `source`, so both identify the instance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FeatureKey {
    pub name: Box<str>,
    pub source: FeatureSource,
}

impl FeatureKey {
    pub fn new(name: impl AsRef<str>, source: FeatureSource) -> Self {
        Self {
            name: Box::from(name.as_ref()),
            source,
        }
    }

    pub fn from_pending(pending: &PendingFeature) -> Self {
        Self::new(&*pending.name, pending.source.clone())
    }
}

/// Per-feature submit data: stored args + optional replacement pick.
/// Keyed by `FeatureKey` in [`ApplyInputs`].
///
/// Records sit at two kinds of keys:
/// - **Placeholder key** (`(ASI, L8)`): when the user (or a recovered prior
///   decision) replaces this slot, `replacement = Some(name)`. Own `inputs` are
///   unused — they belong to the resolved feature, not the placeholder.
/// - **Resolved key** (`(Spell Sniper, L8)`): `inputs` carry the user's
///   ARG/dice picks for the actually-applied feat; `replacement = None`.
///
/// Plain (non-swap) features have a single record at their own key with
/// `replacement = None` and `inputs` populated.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ApplyInput {
    pub inputs: Vec<AssignInputs>,
    pub replacement: Option<Box<str>>,
}

/// Bundled user inputs from the args/dice modal, keyed by `FeatureKey`.
/// Using `(name, source)` as the key lets stackable features with multiple
/// instances (e.g. ASI at Monk L4 and Monk L8) carry distinct inputs and
/// distinct replacement decisions — a name-only map would collapse them
/// into one.
pub type ApplyInputs = BTreeMap<FeatureKey, ApplyInput>;

/// Convert `PendingInputs` prefill into `ApplyInputs` for a silent-commit
/// trial `build_clean`. Each pending lands as one record at its own
/// `(name, source)` key, carrying both prefilled args and the detected
/// replacement — so the rebuild cascade reproduces the user's prior swap
/// decisions.
pub fn synthesize_apply_inputs(pending: &[PendingInputs]) -> ApplyInputs {
    pending
        .iter()
        .map(|pending| {
            let key = FeatureKey::new(&pending.feature_name, pending.source.clone());
            let input = ApplyInput {
                inputs: pending.prefill.clone(),
                replacement: pending.prefilled_replacement.clone(),
            };
            (key, input)
        })
        .collect()
}

/// A feature whose assignment expressions require user interaction (ARG values
/// and/or dice rolls). Each expression in `exprs` gets its own independent
/// ARG context and dice pool.
#[derive(Clone, PartialEq)]
pub struct PendingInputs {
    pub feature_name: Box<str>,
    pub exprs: Vec<Expr>,
    /// Existing stored inputs aligned with `exprs` (by index). Empty when the
    /// feature is being applied for the first time. Used by the modal to
    /// pre-fill ARG and dice signals so re-apply behaves as edit.
    pub prefill: Vec<AssignInputs>,
    pub replace_with: ReplaceWith,
    /// Pre-chosen replacement feature name (e.g. from AI generation,
    /// edit-of-swap-row). When set, `ReplacementPicker` initializes
    /// `replacement_choice` with this value and shows the replacement UI
    /// expanded. User can still override.
    pub prefilled_replacement: Option<Box<str>>,
    /// Pre-filled inputs for the replacement feature's expressions, indexed
    /// by expr position (`replacement_prefill[i]` feeds expr `i`). Empty Vec
    /// = no prefill; positions past the Vec end render as
    /// `AssignInputs::default()`. There is no broadcast or fallthrough — a
    /// short Vec leaves the rest of the exprs explicitly empty rather than
    /// silently reusing one value across multiple exprs.
    pub replacement_prefill: Vec<AssignInputs>,
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

    /// Stable identifier for this pending entry — `(feature_name, source)`.
    /// Used by the modal's `<For>` key extractor and by per-section snapshot
    /// Effects that need to find their own position in the pending list.
    pub fn feature_key(&self) -> FeatureKey {
        FeatureKey::new(&self.feature_name, self.source.clone())
    }

    /// Build a `PendingInputs` from a feature definition. Returns `None` if
    /// the feature has no interactive exprs under `when` and is not
    /// replaceable (by the supplied `replace_with`).
    pub fn from_feature(
        name: Box<str>,
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
            exprs,
            prefill,
            replace_with,
            prefilled_replacement: None,
            replacement_prefill: Vec::new(),
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
        name: Box<str>,
        feat_def: &FeatureDefinition,
        source: FeatureSource,
    ) -> Self {
        Self {
            feature_name: name,
            exprs: Vec::new(),
            prefill: Vec::new(),
            replace_with: feat_def.replace_with,
            prefilled_replacement: None,
            replacement_prefill: Vec::new(),
            source,
            hidden: true,
        }
    }
}

/// A feature pending application. Owned and cheap — survives move closure
/// boundaries (modal callbacks). Produced by collect functions, consumed by
/// apply primitives.
#[derive(Clone, Debug, Default)]
pub struct PendingFeature {
    pub name: Box<str>,
    pub source: FeatureSource,
    pub level: u32,
    /// Placeholder name to record on the resulting row's `replaces`.
    pub replaces: Option<Box<str>>,
}

impl PendingFeature {
    pub fn feature_key(&self) -> FeatureKey {
        FeatureKey::new(&self.name, self.source.clone())
    }
}

impl PendingFeature {
    /// Bridge to PendingInputs for the modal UI. Returns Some if this
    /// feature needs user interaction (ARG values, dice rolls, or is
    /// replaceable).
    pub fn pending_inputs(
        &self,
        feat_def: &FeatureDefinition,
        character: &CharacterCore,
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
