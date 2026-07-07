use std::{borrow::Borrow, collections::BTreeMap, fmt, ops::Deref};

use serde::Deserialize;

use crate::{
    demap::Named,
    rules::{
        background::BackgroundDefinition,
        class::{ClassDefinition, SubclassDefinition},
        feature::{ActionDefinition, ChoiceOption, ChoiceOptions, FeatureDefinition},
        species::SpeciesDefinition,
    },
};

/// A dot-separated key in a locale map.
///
/// Examples: `""`, `"feature.Arcane Recovery"`,
/// `"subclass.School of Evocation.feature.Evocation Savant"`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocaleKey(Box<str>);

impl LocaleKey {
    /// Parse into a structured path for matching.
    pub fn parse(&self) -> LocalePath<'_> {
        if self.0.is_empty() {
            return LocalePath::Root;
        }

        let s = &*self.0;
        match s.find('.') {
            Some(pos) if &s[..pos] == "subclass" => LocalePath::Subclass(&s[pos + 1..]),
            _ => LocalePath::Unknown,
        }
    }
}

/// Parsed locale key path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalePath<'a> {
    /// `""` — the entity itself
    Root,
    /// `"subclass.X"` (may contain nested segments like `.feature.Y`)
    Subclass(&'a str),
    /// Unrecognized path
    Unknown,
}

/// Locale text entry — the value side of a locale map.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LocaleText {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl LocaleText {
    /// Always assign label (clears to None if locale doesn't have one).
    pub fn apply_label(&self, target: &mut Option<String>) {
        *target = self.label.clone();
    }

    /// Always assign description (clears if locale doesn't have one).
    pub fn apply_description(&self, target: &mut String) {
        match &self.description {
            Some(desc) => desc.clone_into(target),
            None => target.clear(),
        }
    }

    /// Returns true if all fields are None/empty.
    pub fn is_empty(&self) -> bool {
        self.label.is_none() && self.description.is_none()
    }
}

/// A complete locale map for one entity file.
pub type LocaleMap = BTreeMap<LocaleKey, LocaleText>;

/// A definition paired with its current-locale text overlay. `Deref<Target =
/// T>` gives consumers transparent access to the underlying definition fields,
/// while `.label()` / `.description()` overlay locale on top of name fallback.
pub struct LocalizedText<'a, T, L = LocaleText> {
    pub data: &'a T,
    pub locale: Option<&'a L>,
}

impl<T, L> Deref for LocalizedText<'_, T, L> {
    type Target = T;

    fn deref(&self) -> &T {
        self.data
    }
}

/// Flat-key flavor: locale entry is a single `LocaleText`.
impl<T: Named> LocalizedText<'_, T, LocaleText> {
    pub fn label(&self) -> &str {
        self.locale
            .and_then(|t| t.label.as_deref())
            .unwrap_or_else(|| self.data.name())
    }

    pub fn description(&self) -> &str {
        self.locale
            .and_then(|t| t.description.as_deref())
            .unwrap_or("")
    }
}

/// Nested-key flavor for the global features index. Locale is the entire
/// `LocaleMap`; bare key is the feature name, sub-keys live behind
/// `flat_field` / `flat_field_option`.
impl<'a> LocalizedText<'a, FeatureDefinition, LocaleMap> {
    pub fn label(&self) -> &str {
        self.locale
            .and_then(|m| m.get(&*self.data.name))
            .and_then(|t| t.label.as_deref())
            .unwrap_or(&*self.data.name)
    }

    pub fn description(&self) -> &str {
        self.locale
            .and_then(|m| m.get(&*self.data.name))
            .and_then(|t| t.description.as_deref())
            .unwrap_or("")
    }

    pub fn action(&'a self, name: &str) -> Option<LocalizedAction<'a>> {
        self.data
            .actions
            .get(name)
            .map(|action_def| LocalizedAction {
                feat_name: &self.data.name,
                data: action_def,
                locale: self.locale,
            })
    }
}

/// Wrapper for an `ActionDefinition` that knows its parent feature name so
/// the flat sub-key (`"FeatName.field.X"`) can be built on demand. The
/// "field" infix is preserved in locale keys for migration continuity.
pub struct LocalizedAction<'a> {
    pub feat_name: &'a str,
    pub data: &'a ActionDefinition,
    pub locale: Option<&'a LocaleMap>,
}

impl<'a> Deref for LocalizedAction<'a> {
    type Target = ActionDefinition;

    fn deref(&self) -> &ActionDefinition {
        self.data
    }
}

impl<'a> LocalizedAction<'a> {
    fn entry(&self) -> Option<&'a LocaleText> {
        let key = LocaleKey::flat_field(self.feat_name, &self.data.name);
        self.locale.and_then(|m| m.get(key.as_str()))
    }

    pub fn label(&self) -> &str {
        self.entry()
            .and_then(|t| t.label.as_deref())
            .unwrap_or(&*self.data.name)
    }

    pub fn description(&self) -> &str {
        self.entry()
            .and_then(|t| t.description.as_deref())
            .unwrap_or("")
    }

    pub fn option(&'a self, opt_name: &str) -> Option<LocalizedOption<'a>> {
        let ChoiceOptions::List(opts) = &self.data.options else {
            return None;
        };
        opts.iter()
            .find(|opt| &*opt.name == opt_name)
            .map(|opt| LocalizedOption {
                feat_name: self.feat_name,
                field_name: &self.data.name,
                data: opt,
                locale: self.locale,
            })
    }
}

/// Wrapper for a `ChoiceOption` (definition) that knows its parent
/// feature/field names so the flat sub-key (`"FeatName.field.X.option.Y"`)
/// can be built on demand.
pub struct LocalizedOption<'a> {
    pub feat_name: &'a str,
    pub field_name: &'a str,
    pub data: &'a ChoiceOption,
    pub locale: Option<&'a LocaleMap>,
}

impl<'a> Deref for LocalizedOption<'a> {
    type Target = ChoiceOption;

    fn deref(&self) -> &ChoiceOption {
        self.data
    }
}

impl<'a> LocalizedOption<'a> {
    fn entry(&self) -> Option<&'a LocaleText> {
        let key = LocaleKey::flat_field_option(self.feat_name, self.field_name, &self.data.name);
        self.locale.and_then(|m| m.get(key.as_str()))
    }

    pub fn label(&self) -> &str {
        self.entry()
            .and_then(|t| t.label.as_deref())
            .unwrap_or(&*self.data.name)
    }

    pub fn description(&self) -> &str {
        self.entry()
            .and_then(|t| t.description.as_deref())
            .unwrap_or("")
    }
}

/// Marker for definitions living in a merged per-kind locale file
/// (classes.json / species.json / backgrounds.json) keyed by entity name,
/// same convention as `FeatureDefinition` in the global features map.
pub trait RootKeyed: Named {}

impl RootKeyed for ClassDefinition {}
impl RootKeyed for SpeciesDefinition {}
impl RootKeyed for BackgroundDefinition {}

impl<T: RootKeyed> LocalizedText<'_, T, LocaleMap> {
    pub fn label(&self) -> &str {
        self.locale
            .and_then(|m| m.get(self.data.name()))
            .and_then(|t| t.label.as_deref())
            .unwrap_or(self.data.name())
    }

    pub fn description(&self) -> &str {
        self.locale
            .and_then(|m| m.get(self.data.name()))
            .and_then(|t| t.description.as_deref())
            .unwrap_or("")
    }
}

/// `ClassDefinition` extra: subclasses keyed by `{Class}.subclass.{Sub}`.
impl<'a> LocalizedText<'a, ClassDefinition, LocaleMap> {
    pub fn subclass(&'a self, name: &str) -> Option<LocalizedSubclass<'a>> {
        self.data
            .subclasses
            .get(name)
            .map(|sub_def| LocalizedSubclass {
                parent: self.data.name(),
                data: sub_def,
                locale: self.locale,
            })
    }
}

/// Wrapper for a `SubclassDefinition` keyed by `{parent}.subclass.{name}`
/// in the merged classes locale map.
pub struct LocalizedSubclass<'a> {
    pub parent: &'a str,
    pub data: &'a SubclassDefinition,
    pub locale: Option<&'a LocaleMap>,
}

impl<'a> Deref for LocalizedSubclass<'a> {
    type Target = SubclassDefinition;

    fn deref(&self) -> &SubclassDefinition {
        self.data
    }
}

impl<'a> LocalizedSubclass<'a> {
    fn entry(&self) -> Option<&'a LocaleText> {
        let key = format!("{}.subclass.{}", self.parent, self.data.name);
        self.locale.and_then(|m| m.get(key.as_str()))
    }

    pub fn label(&self) -> &str {
        self.entry()
            .and_then(|t| t.label.as_deref())
            .unwrap_or(&*self.data.name)
    }

    pub fn description(&self) -> &str {
        self.entry()
            .and_then(|t| t.description.as_deref())
            .unwrap_or("")
    }
}

// --- Deserialization ---

impl<'de> Deserialize<'de> for LocaleKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = Box::<str>::deserialize(deserializer)?;
        Ok(Self(s))
    }
}

// --- Display ---

impl fmt::Display for LocaleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Borrow<str> for LocaleKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl LocaleKey {
    /// Whether this key is a bare name (no dots).
    pub fn is_bare(&self) -> bool {
        !self.0.contains('.')
    }

    /// The raw key string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Flat key for the features index (no "feature." prefix).
    pub fn flat_field(feat: &str, field: &str) -> Self {
        Self(format!("{feat}.field.{field}").into_boxed_str())
    }

    pub fn flat_field_option(feat: &str, field: &str, option: &str) -> Self {
        Self(format!("{feat}.field.{field}.option.{option}").into_boxed_str())
    }
}

// --- Index and effects locale application ---

/// Index locale map: keys like "class.Wizard", "species.Tiefling", etc.
pub type IndexLocaleMap = BTreeMap<Box<str>, LocaleText>;

/// Effects locale map: keys are effect names.
pub type EffectsLocaleMap = BTreeMap<Box<str>, LocaleText>;

/// Spells overlay map: keys are spell names.
pub type SpellsLocaleMap = BTreeMap<Box<str>, LocaleText>;
