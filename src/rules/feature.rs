use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    demap::{self, Named},
    expr::Eval as _,
    model::{
        ActionType, Attribute, Character, EffectDefinition, Expr, FeatureCategory, FeatureField,
        Translatable, short_name,
    },
    rules::spells::SpellsDefinition,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub enum ReplaceWith {
    #[default]
    None,
    Any,
    Category(FeatureCategory),
}

impl ReplaceWith {
    pub fn matches(&self, feat: &FeatureDefinition) -> bool {
        match self {
            Self::None => false,
            Self::Any => feat.is_selectable(),
            Self::Category(cat) => feat.category == *cat,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeatureDefinition {
    pub name: Box<str>,
    #[serde(default)]
    pub stackable: bool,
    #[serde(default)]
    pub category: FeatureCategory,
    #[serde(default)]
    pub replace_with: ReplaceWith,
    pub spells: Option<SpellsDefinition>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub actions: BTreeMap<Box<str>, ActionDefinition>,
    #[serde(default)]
    pub assign: Option<Vec<Assignment>>,
    #[serde(default)]
    pub prerequisites: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub expr: Expr,
    pub when: WhenCondition,
}

impl Assignment {
    /// Structural check: the expression references `@ARG` / `Arg(_)` or
    /// contains dice rolls. Such assignments consume user-supplied
    /// `AssignInputs` at apply time; non-interactive assignments run
    /// against the raw character context only.
    pub fn is_interactive(&self) -> bool {
        self.expr.has_var(|var| matches!(var, Attribute::Arg(_))) || self.expr.has_dice()
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString
)]
pub enum WhenCondition {
    /// Feature pipeline: runs once when feature is added.
    OnFeatureAdd,
    /// Feature pipeline: runs every `Character::compute()` cycle, mutates
    /// character base.
    OnCompute,
    /// Gear pipeline: runs every `Character::compute()` cycle while the gear
    /// is active. Mutates `<gear>.magic.charges` and may mutate character base.
    OnGearActive,
    /// Gear pipeline: runs every `ActiveEffects::recompute()` cycle while the
    /// gear is active. Writes to overrides only.
    OnEffect,
    /// Either pipeline: long-rest event.
    OnLongRest,
    /// Either pipeline: short-rest event.
    OnShortRest,
}

impl Translatable for WhenCondition {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::OnFeatureAdd => "when-on-feature-add",
            Self::OnCompute => "when-on-compute",
            Self::OnGearActive => "when-on-gear-active",
            Self::OnEffect => "when-on-effect",
            Self::OnLongRest => "when-on-long-rest",
            Self::OnShortRest => "when-on-short-rest",
        }
    }
}

impl Named for FeatureDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Global features index, loaded from `features.json`.
#[derive(Clone)]
pub struct FeaturesIndex(pub BTreeMap<Box<str>, FeatureDefinition>);

/// Empty fallback for callers that need a stable reference when the index
/// hasn't loaded yet (e.g. native tests, recompute before features.json is in).
pub static EMPTY_FEATURES_INDEX: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();

impl<'de> serde::Deserialize<'de> for FeaturesIndex {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        demap::named_map(deserializer).map(Self)
    }
}

impl FeatureDefinition {
    pub fn is_selectable(&self) -> bool {
        !matches!(self.category, FeatureCategory::Class)
    }

    pub fn is_replaceable(&self) -> bool {
        !matches!(self.replace_with, ReplaceWith::None)
    }

    pub fn meets_prerequisites(&self, character: &Character) -> bool {
        self.prerequisites
            .as_ref()
            .is_none_or(|expr| expr.eval(character).unwrap_or(0) != 0)
    }

    /// Structural check: does this feature have any @ARG / dice assignments
    /// that require user input through the args modal? Replaceable features
    /// (subclass picks, epic boon replacement) also count as interactive.
    pub fn has_interactive_inputs(&self) -> bool {
        self.is_replaceable()
            || self
                .assign
                .as_ref()
                .is_some_and(|assignments| assignments.iter().any(|assign| assign.is_interactive()))
    }

    /// Returns `(cost_field_name, short_suffix)` if this feature has a
    /// spells cost referencing a named pool (e.g. Sorcery Points → "SP").
    pub fn cost_info(&self) -> Option<(&str, String)> {
        let cost_name = self.spells.as_ref()?.cost.as_deref()?;
        let short = short_name(cost_name);
        Some((cost_name, short))
    }

    /// Resolve `ChoiceOptions` to definition options, following `Ref` links
    /// within this feature's actions.
    pub fn resolve_def_options<'a>(&'a self, options: &'a ChoiceOptions) -> &'a [ChoiceOption] {
        match options {
            ChoiceOptions::List(list) => list.as_slice(),
            ChoiceOptions::Ref { from } => self
                .actions
                .get(from.as_str())
                .and_then(|ref_action| match &ref_action.options {
                    ChoiceOptions::List(list) => Some(list.as_slice()),
                    _ => None,
                })
                .unwrap_or(&[]),
        }
    }

    /// Returns `true` if any assignment for the given condition references
    /// `ARG.n` variables (meaning the user must supply arguments before apply).
    pub fn needs_args(&self, when: WhenCondition) -> bool {
        self.args_exprs(when).next().is_some()
    }

    /// Returns all assignment expressions for the given condition that use
    /// `ARG.n` variables. Each gets its own independent ARG context in the UI.
    pub fn args_exprs(&self, when: WhenCondition) -> impl Iterator<Item = &Expr> + '_ {
        self.assign
            .iter()
            .flatten()
            .filter(move |assign| {
                assign.when == when && assign.expr.has_var(|v| matches!(v, Attribute::Arg(_)))
            })
            .map(|assign| &assign.expr)
    }

    /// Returns assignment expressions for the given condition that could
    /// structurally need user interaction — i.e. reference an ARG variable
    /// or contain dice rolls somewhere in the IR. Context-dependent pruning
    /// (which ARGs are actually reachable given current guards) is done
    /// per-snapshot by the cascade chain in the args modal.
    ///
    /// Over-inclusive by design: an expr whose ARG is guarded-unreachable in
    /// every possible character state will reach the modal, where the per-
    /// snapshot `ExprAnalysis` computes empty `active_args` and the modal
    /// renders a "no-eligible-options" placeholder instead of inputs.
    pub fn interactive_exprs(&self, when: WhenCondition) -> Vec<Expr> {
        self.assign
            .iter()
            .flatten()
            .filter(|assignment| assignment.when == when && assignment.is_interactive())
            .map(|assignment| assignment.expr.clone())
            .collect()
    }
}

/// A user-facing Choice slot on a feature: named action with selectable
/// options and optional cost-pool linkage. Per-level option counts are
/// driven by `CHOICE.<name>.COUNT` assignments, not the definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDefinition {
    pub name: Box<str>,
    #[serde(default)]
    pub options: ChoiceOptions,
    #[serde(default)]
    pub cost: Option<String>,
}

impl ActionDefinition {
    /// Resolve `ChoiceOptions` to the concrete options visible at the given
    /// class level. `Ref { from }` follows another action's stored selections.
    pub fn resolve_choice_options(
        &self,
        character_fields: &[FeatureField],
        class_level: u32,
    ) -> Vec<ChoiceOption> {
        match &self.options {
            ChoiceOptions::List(list) => list
                .iter()
                .filter(|opt| opt.level <= class_level)
                .cloned()
                .collect(),
            ChoiceOptions::Ref { from } => character_fields
                .iter()
                .find(|field| field.name == *from)
                .into_iter()
                .flat_map(|field| field.value.choices())
                .filter(|opt| !opt.name.is_empty())
                .map(|opt| ChoiceOption {
                    name: opt.name.clone().into_boxed_str(),
                    label: opt.label.clone(),
                    description: opt.description.clone(),
                    cost: opt.cost,
                    consumes: 0,
                    level: 0,
                    action: None,
                    effects: Vec::new(),
                })
                .collect(),
        }
    }
}

impl Named for ActionDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChoiceOptions {
    List(Vec<ChoiceOption>),
    Ref { from: String },
}

impl Default for ChoiceOptions {
    fn default() -> Self {
        Self::List(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub name: Box<str>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cost: u32,
    /// Items consumed per use (Item.quantity decrement). Items only — for
    /// feature actions this is always 0.
    #[serde(default)]
    pub consumes: u32,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub action: Option<ActionType>,
    #[serde(default)]
    pub effects: Vec<EffectDefinition>,
}

impl ChoiceOption {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

impl Named for ChoiceOption {
    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_features_json() {
        let data = include_str!("../../public/data/features.json");
        let index: FeaturesIndex = serde_json::from_str(data)
            .expect("features.json should deserialize into FeaturesIndex");
        assert!(
            index.0.len() > 900,
            "expected 900+ features, got {}",
            index.0.len()
        );
    }
}
