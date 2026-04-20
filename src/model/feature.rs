use std::{
    collections::BTreeMap,
    fmt,
    ops::{Deref, DerefMut},
};

use leptos_fluent::I18n;
use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use crate::{
    expr::DicePool,
    model::{Die, SpellData, Translatable},
};

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    EnumIter,
    Display,
    EnumString,
    Serialize,
    Deserialize
)]
pub enum FeatureCategory {
    #[default]
    Class,
    Origin,
    General,
    FightingStyle,
    EpicBoon,
    Generation,
    Faction,
    Dragonmark,
}

impl Translatable for FeatureCategory {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::Class => "feat-cat-class",
            Self::Origin => "feat-cat-origin",
            Self::General => "feat-cat-general",
            Self::FightingStyle => "feat-cat-fighting-style",
            Self::EpicBoon => "feat-cat-epic-boon",
            Self::Generation => "feat-cat-generation",
            Self::Faction => "feat-cat-faction",
            Self::Dragonmark => "feat-cat-dragonmark",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Feature {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub applied: bool,
    #[serde(default)]
    pub category: FeatureCategory,
    #[serde(default)]
    pub source: FeatureSource,
    #[serde(default)]
    pub inputs: Vec<AssignInputs>,
}

impl Feature {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn set_label(&mut self, value: String) {
        self.label = Some(value);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FeatureSource {
    Class(Box<str>, u32),
    Subclass(Box<str>, Box<str>, u32),
    #[serde(alias = "Race")]
    Species(Box<str>),
    Background(Box<str>),
    User(u32),
}

impl Default for FeatureSource {
    fn default() -> Self {
        Self::User(0)
    }
}

impl fmt::Display for FeatureSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Class(name, level) => write!(f, "Class: {name} ({level})"),
            Self::Subclass(class_name, subclass_name, level) => {
                write!(f, "Subclass: {class_name} - {subclass_name} ({level})")
            }
            Self::Species(name) => write!(f, "Species: {name}"),
            Self::Background(name) => write!(f, "Background: {name}"),
            Self::User(level) => write!(f, "User ({level})"),
        }
    }
}

impl FeatureSource {
    pub fn display_name(&self, i18n: I18n) -> Option<String> {
        match self {
            Self::Class(name, level) => {
                let prefix = i18n.tr("source-class");
                Some(format!("{prefix}: {name} ({level})"))
            }
            Self::Subclass(class, name, level) => {
                let prefix = i18n.tr("source-subclass");
                Some(format!("{prefix}: {class} — {name} ({level})"))
            }
            Self::Species(name) => {
                let prefix = i18n.tr("source-species");
                Some(format!("{prefix}: {name}"))
            }
            Self::Background(name) => {
                let prefix = i18n.tr("source-background");
                Some(format!("{prefix}: {name}"))
            }
            Self::User(_) => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Class(name, _)
            | Self::Species(name)
            | Self::Background(name)
            | Self::Subclass(_, name, _) => name,
            Self::User(_) => "",
        }
    }

    pub fn is_user(&self) -> bool {
        matches!(self, Self::User(_))
    }

    pub fn as_class(&self) -> Option<&str> {
        match self {
            Self::Class(name, _) => Some(name),
            Self::Subclass(name, _, _) => Some(name),
            _ => None,
        }
    }

    pub fn added_at_level(&self) -> u32 {
        match self {
            Self::Class(_, level) | Self::User(level) | Self::Subclass(_, _, level) => *level,
            Self::Species(_) | Self::Background(_) => 1,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Store)]
#[serde(transparent)]
pub struct FeatureList(Vec<Feature>);

impl Deref for FeatureList {
    type Target = Vec<Feature>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FeatureList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a FeatureList {
    type IntoIter = std::slice::Iter<'a, Feature>;
    type Item = &'a Feature;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut FeatureList {
    type IntoIter = std::slice::IterMut<'a, Feature>;
    type Item = &'a mut Feature;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl From<Vec<Feature>> for FeatureList {
    fn from(features: Vec<Feature>) -> Self {
        Self(features)
    }
}

impl FeatureList {
    /// Does this feature definition + source already have an applied instance?
    /// Non-stackable: any applied by name → true.
    /// Stackable: applied with same name AND source → true.
    pub fn contains(&self, name: &str, stackable: bool, source: &FeatureSource) -> bool {
        if stackable {
            self.0
                .iter()
                .any(|f| f.name == name && f.applied && f.source == *source)
        } else {
            self.0.iter().any(|f| f.name == name && f.applied)
        }
    }

    pub fn has(&self, name: &str) -> bool {
        self.0.iter().any(|f| f.name == name && f.applied)
    }

    pub fn has_category(&self, category: FeatureCategory) -> bool {
        self.0.iter().any(|f| f.applied && f.category == category)
    }

    /// Is this a first-time add (OnFeatureAdd)?
    /// True if no entries at all, or has an unapplied entry waiting.
    pub fn is_pending(&self, name: &str) -> bool {
        let mut has_applied = false;
        let mut has_unapplied = false;
        for feature in &self.0 {
            if feature.name == name {
                if feature.applied {
                    has_applied = true;
                } else {
                    has_unapplied = true;
                }
            }
        }
        !has_applied || has_unapplied
    }

    /// Add a feature with its inputs. Finds an unapplied entry and fills
    /// it in, or pushes a new applied entry (for stackable features from a
    /// different source, or brand new features).
    pub fn add(
        &mut self,
        name: &str,
        label: Option<String>,
        description: String,
        category: FeatureCategory,
        source: FeatureSource,
        inputs: Vec<AssignInputs>,
    ) {
        if let Some(feature) = self.0.iter_mut().rfind(|f| f.name == name && !f.applied) {
            feature.applied = true;
            feature.label = label;
            feature.description = description;
            feature.category = category;
            feature.source = source;
            feature.inputs = inputs;
        } else {
            self.0.push(Feature {
                name: name.to_string(),
                label,
                description,
                applied: true,
                category,
                source,
                inputs,
            });
        }
    }

    /// Look up stored inputs for a feature by name.
    pub fn get_inputs(&self, name: &str) -> &[AssignInputs] {
        self.0
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.inputs.as_slice())
            .unwrap_or_default()
    }
}

/// Container joining the applied feature list with per-feature data
/// (fields + spells). Co-located because the two are tightly coupled —
/// `data` entries are keyed by feature name and should exist for every
/// applied feature that has fields or spells.
///
/// `Deref`/`DerefMut` target the `BTreeMap<String, FeatureData>` data map
/// so map-style access (`.get()`, `.get_mut()`, `.insert()`, `.keys()`,
/// `.values()`, `.clear()`) works through the container directly. List
/// iteration and list-specific queries go through inherent methods
/// (`iter`, `contains`, `has`, `is_pending`, `add`, `get_inputs`,
/// `has_category`) which override the BTreeMap accessors where names
/// clash.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Store)]
pub struct Features {
    #[serde(default)]
    pub list: FeatureList,
    #[serde(default)]
    data: BTreeMap<String, FeatureData>,
}

impl Deref for Features {
    type Target = BTreeMap<String, FeatureData>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for Features {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a> IntoIterator for &'a Features {
    type IntoIter = std::slice::Iter<'a, Feature>;
    type Item = &'a Feature;

    fn into_iter(self) -> Self::IntoIter {
        self.list.iter()
    }
}

impl<'a> IntoIterator for &'a mut Features {
    type IntoIter = std::slice::IterMut<'a, Feature>;
    type Item = &'a mut Feature;

    fn into_iter(self) -> Self::IntoIter {
        self.list.iter_mut()
    }
}

impl Features {
    /// Construct from an existing list + data pair. Only used in tests where
    /// struct literals would otherwise need access to the private `data`
    /// field.
    #[cfg(test)]
    pub fn from_parts(list: FeatureList, data: BTreeMap<String, FeatureData>) -> Self {
        Self { list, data }
    }

    /// Iterate applied feature list (overrides BTreeMap::iter from Deref).
    pub fn iter(&self) -> std::slice::Iter<'_, Feature> {
        self.list.iter()
    }

    /// Mutable iter over list (overrides BTreeMap::iter_mut from Deref).
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Feature> {
        self.list.iter_mut()
    }

    // List-method delegates — same signatures as FeatureList.

    pub fn contains(&self, name: &str, stackable: bool, source: &FeatureSource) -> bool {
        self.list.contains(name, stackable, source)
    }

    pub fn has(&self, name: &str) -> bool {
        self.list.has(name)
    }

    pub fn has_category(&self, category: FeatureCategory) -> bool {
        self.list.has_category(category)
    }

    pub fn is_pending(&self, name: &str) -> bool {
        self.list.is_pending(name)
    }

    pub fn get_inputs(&self, name: &str) -> &[AssignInputs] {
        self.list.get_inputs(name)
    }

    pub fn add(
        &mut self,
        name: &str,
        label: Option<String>,
        description: String,
        category: FeatureCategory,
        source: FeatureSource,
        inputs: Vec<AssignInputs>,
    ) {
        self.list
            .add(name, label, description, category, source, inputs);
    }

    // Raw data accessors for explicit BTreeMap access where Deref coercion
    // isn't convenient (e.g. cloning, passing as function argument).

    pub fn data(&self) -> &BTreeMap<String, FeatureData> {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut BTreeMap<String, FeatureData> {
        &mut self.data
    }

    // Per-feature data helpers.

    /// Shorthand for `data.get(name).and_then(|e| e.spells.as_ref())`.
    pub fn spell_data(&self, name: &str) -> Option<&SpellData> {
        self.data.get(name).and_then(|e| e.spells.as_ref())
    }

    /// Shorthand for `data.get_mut(name).and_then(|e| e.spells.as_mut())`.
    pub fn spell_data_mut(&mut self, name: &str) -> Option<&mut SpellData> {
        self.data.get_mut(name).and_then(|e| e.spells.as_mut())
    }

    /// Zero `used` on every Points/Die field and every spell's `free_uses`
    /// across all data entries. Used by `Character::long_rest`.
    pub fn reset_uses(&mut self) {
        for entry in self.data.values_mut() {
            for field in &mut entry.fields {
                match &mut field.value {
                    FeatureValue::Points { used, .. } | FeatureValue::Die { used, .. } => {
                        *used = 0;
                    }
                    _ => {}
                }
            }
            if let Some(spell_data) = entry.spells.as_mut() {
                for spell in &mut spell_data.spells {
                    if let Some(fu) = spell.free_uses.as_mut() {
                        fu.used = 0;
                    }
                }
            }
        }
    }

    /// Clear labels and descriptions on list entries and all data fields/
    /// spells. Used by `Character::clear_all_labels`.
    pub fn clear_all_labels(&mut self) {
        for feature in &mut self.list {
            feature.label = None;
            feature.description.clear();
        }
        for entry in self.data.values_mut() {
            for field in &mut entry.fields {
                field.label = None;
                field.description.clear();
                for opt in field.value.choices_mut() {
                    opt.label = None;
                    opt.description.clear();
                }
            }
            if let Some(spells) = &mut entry.spells {
                for spell in &mut spells.spells {
                    spell.label = None;
                    spell.description.clear();
                }
                for spell in spells.known.iter_mut().flatten() {
                    spell.label = None;
                    spell.description.clear();
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct AssignInputs {
    #[serde(default)]
    pub args: Vec<i32>,
    #[serde(default)]
    pub dice: DicePool,
}

impl AssignInputs {
    pub fn is_empty(&self) -> bool {
        self.args.is_empty() && self.dice.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct FeatureData {
    #[serde(default)]
    pub fields: Vec<FeatureField>,
    #[serde(default)]
    pub spells: Option<SpellData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct FeatureField {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub value: FeatureValue,
}

impl FeatureField {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub enum FeatureValue {
    Points { used: u32, max: u32 },
    Choice { options: Vec<FeatureOption> },
    Die { die: Die, used: u32 },
    Bonus(i32),
}

impl Default for FeatureValue {
    fn default() -> Self {
        FeatureValue::Points { used: 0, max: 0 }
    }
}

/// Derive a short abbreviation from a name by taking the first letter of each
/// word. "Channel Divinity" → "CD", "Sorcery Points" → "SP", "Rages" → "R"
pub fn short_name(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .collect()
}

impl FeatureValue {
    pub fn available_points(&self) -> Option<u32> {
        match self {
            FeatureValue::Points { used, max } => Some(max.saturating_sub(*used)),
            FeatureValue::Die { die, used } => Some(die.amount.saturating_sub(*used)),
            _ => None,
        }
    }

    pub fn max_points(&self) -> Option<u32> {
        match self {
            FeatureValue::Points { max, .. } => Some(*max),
            FeatureValue::Die { die, .. } => Some(die.amount),
            _ => None,
        }
    }

    pub fn choices(&self) -> &[FeatureOption] {
        match self {
            FeatureValue::Choice { options } => options,
            _ => &[],
        }
    }

    pub fn choices_mut(&mut self) -> &mut [FeatureOption] {
        match self {
            FeatureValue::Choice { options } => options,
            _ => &mut [],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct FeatureOption {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cost: u32,
}

impl FeatureOption {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn set_label(&mut self, value: String) {
        self.label = Some(value);
    }
}
