use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, ser::SerializeMap};

use super::{
    background::BackgroundDefinition,
    class::ClassDefinition,
    feature::{ChoiceOptions, FeatureDefinition, FeaturesIndex, FieldKind},
    index::Index,
    race::RaceDefinition,
    spells::SpellMap,
};
use crate::model::EffectsIndex;

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
        // Split off first segment
        let (kind, rest) = split_first(s);
        match kind {
            "feature" => parse_feature(rest.unwrap_or("")),
            "subclass" => parse_subclass(rest.unwrap_or("")),
            _ => LocalePath::Unknown,
        }
    }
}

/// Split `s` at the first `.` that separates a keyword from the rest.
/// For keywords like "feature", "subclass", "field", "option", "spell",
/// the split is at the first dot after the keyword.
fn split_first(s: &str) -> (&str, Option<&str>) {
    match s.find('.') {
        Some(pos) => (&s[..pos], Some(&s[pos + 1..])),
        None => (s, None),
    }
}

/// Split `rest` at ".field.", ".spell.", etc. — compound keyword boundaries.
/// Returns (name, remaining) where name may contain dots (e.g. entity names
/// with dots).
fn split_at_keyword<'a>(s: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let needle = format!(".{keyword}.");
    s.find(&needle)
        .map(|pos| (&s[..pos], &s[pos + needle.len()..]))
}

fn parse_feature(rest: &str) -> LocalePath<'_> {
    if rest.is_empty() {
        return LocalePath::Unknown;
    }
    // Check for .field. or .spell. within the rest
    if let Some((feat_name, after_field)) = split_at_keyword(rest, "field") {
        return parse_field(
            feat_name,
            after_field,
            LocalePath::FeatureField,
            LocalePath::FeatureFieldOption,
        );
    }
    if let Some((feat_name, spell_name)) = split_at_keyword(rest, "spell") {
        return if spell_name.is_empty() {
            LocalePath::Unknown
        } else {
            LocalePath::FeatureSpell(feat_name, spell_name)
        };
    }
    // Plain feature name
    LocalePath::Feature(rest)
}

fn parse_subclass(rest: &str) -> LocalePath<'_> {
    if rest.is_empty() {
        return LocalePath::Unknown;
    }
    // Check for .feature. within the rest
    if let Some((sc_name, after_feature)) = split_at_keyword(rest, "feature") {
        if after_feature.is_empty() {
            return LocalePath::Unknown;
        }
        // Now after_feature is the subclass feature part — check for .field. within it
        if let Some((feat_name, after_field)) = split_at_keyword(after_feature, "field") {
            return parse_field(
                feat_name,
                after_field,
                |feat, field| LocalePath::SubclassFeatureField(sc_name, feat, field),
                |feat, field, opt| {
                    LocalePath::SubclassFeatureFieldOption(sc_name, feat, field, opt)
                },
            );
        }
        return LocalePath::SubclassFeature(sc_name, after_feature);
    }
    // Plain subclass name
    LocalePath::Subclass(rest)
}

fn parse_field<'a>(
    owner_name: &'a str,
    after_field: &'a str,
    make_field: impl FnOnce(&'a str, &'a str) -> LocalePath<'a>,
    make_option: impl FnOnce(&'a str, &'a str, &'a str) -> LocalePath<'a>,
) -> LocalePath<'a> {
    if after_field.is_empty() {
        return LocalePath::Unknown;
    }
    // Check for .option. within the field part
    if let Some((field_name, option_name)) = split_at_keyword(after_field, "option") {
        return if option_name.is_empty() {
            LocalePath::Unknown
        } else {
            make_option(owner_name, field_name, option_name)
        };
    }
    make_field(owner_name, after_field)
}

/// Parsed locale key path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalePath<'a> {
    /// `""` — the entity itself
    Root,
    /// `"feature.X"`
    Feature(&'a str),
    /// `"feature.X.field.Y"`
    FeatureField(&'a str, &'a str),
    /// `"feature.X.field.Y.option.Z"`
    FeatureFieldOption(&'a str, &'a str, &'a str),
    /// `"feature.X.spell.Y"`
    FeatureSpell(&'a str, &'a str),
    /// `"subclass.X"`
    Subclass(&'a str),
    /// `"subclass.X.feature.Y"`
    SubclassFeature(&'a str, &'a str),
    /// `"subclass.X.feature.Y.field.Z"`
    SubclassFeatureField(&'a str, &'a str, &'a str),
    /// `"subclass.X.feature.Y.field.Z.option.W"`
    SubclassFeatureFieldOption(&'a str, &'a str, &'a str, &'a str),
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

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

// --- Serialization (for migration script) ---

impl serde::Serialize for LocaleKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl serde::Serialize for LocaleText {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Count non-None fields
        let count = self.label.is_some() as usize + self.description.is_some() as usize;
        let mut map = serializer.serialize_map(Some(count))?;
        if let Some(label) = &self.label {
            map.serialize_entry("label", label)?;
        }
        if let Some(desc) = &self.description {
            map.serialize_entry("description", desc)?;
        }
        map.end()
    }
}

// --- Construction helpers (for migration script) ---

impl LocaleKey {
    pub fn root() -> Self {
        Self(String::new().into_boxed_str())
    }

    pub fn feature(name: &str) -> Self {
        Self(format!("feature.{name}").into_boxed_str())
    }

    /// Whether this key is a bare name (no dots).
    pub fn is_bare(&self) -> bool {
        !self.0.contains('.')
    }

    /// The raw key string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Flat key for the features catalog (no "feature." prefix).
    pub fn flat_field(feat: &str, field: &str) -> Self {
        Self(format!("{feat}.field.{field}").into_boxed_str())
    }

    pub fn flat_field_option(feat: &str, field: &str, option: &str) -> Self {
        Self(format!("{feat}.field.{field}.option.{option}").into_boxed_str())
    }

    pub fn flat_spell(feat: &str, spell: &str) -> Self {
        Self(format!("{feat}.spell.{spell}").into_boxed_str())
    }

    pub fn feature_field(feat: &str, field: &str) -> Self {
        Self(format!("feature.{feat}.field.{field}").into_boxed_str())
    }

    pub fn feature_field_option(feat: &str, field: &str, option: &str) -> Self {
        Self(format!("feature.{feat}.field.{field}.option.{option}").into_boxed_str())
    }

    pub fn feature_spell(feat: &str, spell: &str) -> Self {
        Self(format!("feature.{feat}.spell.{spell}").into_boxed_str())
    }

    pub fn subclass(name: &str) -> Self {
        Self(format!("subclass.{name}").into_boxed_str())
    }

    pub fn subclass_feature(sc: &str, feat: &str) -> Self {
        Self(format!("subclass.{sc}.feature.{feat}").into_boxed_str())
    }

    pub fn subclass_feature_field(sc: &str, feat: &str, field: &str) -> Self {
        Self(format!("subclass.{sc}.feature.{feat}.field.{field}").into_boxed_str())
    }

    pub fn subclass_feature_field_option(sc: &str, feat: &str, field: &str, option: &str) -> Self {
        Self(format!("subclass.{sc}.feature.{feat}.field.{field}.option.{option}").into_boxed_str())
    }
}

// --- Application to definition types ---

/// Helper: apply locale text to a feature definition's fields and nested items.
fn apply_locale_to_feature(
    feat: &mut FeatureDefinition,
    feat_name: &str,
    locale: &LocaleMap,
    key_prefix: impl Fn(&str, &str) -> LocaleKey,
    key_prefix_option: impl Fn(&str, &str, &str) -> LocaleKey,
    key_prefix_spell: impl Fn(&str, &str) -> LocaleKey,
) {
    // Field labels/descriptions
    for (field_name, field_def) in &mut feat.fields {
        let field_key = key_prefix(feat_name, field_name);
        if let Some(text) = locale.get(&field_key) {
            text.apply_label(&mut field_def.label);
            text.apply_description(&mut field_def.description);
        }

        // Choice option labels/descriptions
        if let FieldKind::Choice {
            options: ChoiceOptions::List(opts),
            ..
        } = &mut field_def.kind
        {
            for opt in opts {
                let opt_key = key_prefix_option(feat_name, field_name, &opt.name);
                if let Some(text) = locale.get(&opt_key) {
                    text.apply_label(&mut opt.label);
                    text.apply_description(&mut opt.description);
                }
            }
        }
    }

    // Inline spell labels/descriptions
    if let Some(spells_def) = &mut feat.spells
        && let super::spells::SpellList::Inline(spell_map) = &mut spells_def.list
    {
        for (spell_name, spell_def) in spell_map.0.iter_mut() {
            let spell_key = key_prefix_spell(feat_name, spell_name);
            if let Some(text) = locale.get(&spell_key) {
                text.apply_label(&mut spell_def.label);
                text.apply_description(&mut spell_def.description);
            }
        }
    }
}

/// Apply a locale map to a `ClassDefinition`.
/// Features are now in the global features catalog, so only root and subclass
/// labels are handled here.
pub fn apply_class_locale(def: &mut ClassDefinition, locale: &LocaleMap) {
    for (key, text) in locale {
        match key.parse() {
            LocalePath::Root => {
                text.apply_label(&mut def.label);
                text.apply_description(&mut def.description);
            }
            LocalePath::Subclass(name) => {
                if let Some(sc) = def.subclasses.get_mut(name) {
                    text.apply_label(&mut sc.label);
                    text.apply_description(&mut sc.description);
                }
            }
            // Features are now in the global features catalog — locale is
            // applied via apply_features_locale instead.
            _ => {}
        }
    }
}

/// Apply a locale map to a `RaceDefinition`.
pub fn apply_race_locale(def: &mut RaceDefinition, locale: &LocaleMap) {
    for (key, text) in locale {
        if key.parse() == LocalePath::Root {
            text.apply_label(&mut def.label);
            text.apply_description(&mut def.description);
        }
    }
}

/// Apply a locale map to a `BackgroundDefinition`.
pub fn apply_background_locale(def: &mut BackgroundDefinition, locale: &LocaleMap) {
    for (key, text) in locale {
        if key.parse() == LocalePath::Root {
            text.apply_label(&mut def.label);
            text.apply_description(&mut def.description);
        }
    }
}

/// Apply a flat name→text locale map to a spell list.
/// Spell locale files use simple name keys (no "feature." prefix).
pub fn apply_spell_locale(spells: &mut SpellMap, locale: &BTreeMap<Box<str>, LocaleText>) {
    for (name, spell_def) in spells.0.iter_mut() {
        if let Some(text) = locale.get(name) {
            text.apply_label(&mut spell_def.label);
            text.apply_description(&mut spell_def.description);
        }
    }
}

// --- Extraction helpers (for migration script) ---

impl LocaleText {
    /// Extract label and description from a named+described entity,
    /// clearing the source fields.
    pub fn extract(label: &mut Option<String>, description: &mut String) -> Self {
        let mut text = Self::new();
        text.label = label.take();
        if !description.is_empty() {
            text.description = Some(std::mem::take(description));
        }
        text
    }
}

/// Extract locale text from a `ClassDefinition`, returning the locale map
/// and leaving the definition with empty text fields.
/// Features are now in the global features catalog, so only root and subclass
/// labels are extracted here.
pub fn extract_class_locale(def: &mut ClassDefinition) -> LocaleMap {
    let mut map = LocaleMap::new();

    // Root
    let root = LocaleText::extract(&mut def.label, &mut def.description);
    if !root.is_empty() {
        map.insert(LocaleKey::root(), root);
    }

    // Subclasses (label/description only — features are in the global catalog)
    for (sc_name, sc) in &mut def.subclasses {
        let sc_text = LocaleText::extract(&mut sc.label, &mut sc.description);
        if !sc_text.is_empty() {
            map.insert(LocaleKey::subclass(sc_name), sc_text);
        }
    }

    map
}

/// Extract locale text from a `RaceDefinition`.
pub fn extract_race_locale(def: &mut RaceDefinition) -> LocaleMap {
    let mut map = LocaleMap::new();

    let root = LocaleText::extract(&mut def.label, &mut def.description);
    if !root.is_empty() {
        map.insert(LocaleKey::root(), root);
    }

    map
}

/// Extract locale text from a `BackgroundDefinition`.
pub fn extract_background_locale(def: &mut BackgroundDefinition) -> LocaleMap {
    let mut map = LocaleMap::new();

    let root = LocaleText::extract(&mut def.label, &mut def.description);
    if !root.is_empty() {
        map.insert(LocaleKey::root(), root);
    }

    map
}

/// Extract locale text from a spell list, returning a flat name→text map.
pub fn extract_spell_locale(spells: &mut SpellMap) -> BTreeMap<Box<str>, LocaleText> {
    let mut map = BTreeMap::new();
    for (name, spell_def) in spells.0.iter_mut() {
        let text = LocaleText::extract(&mut spell_def.label, &mut spell_def.description);
        if !text.is_empty() {
            map.insert(name.clone(), text);
        }
    }
    map
}

// --- Index and effects locale application ---

/// Index locale map: keys like "class.Wizard", "race.Tiefling", etc.
pub type IndexLocaleMap = BTreeMap<Box<str>, LocaleText>;

/// Effects locale map: keys are effect names.
pub type EffectsLocaleMap = BTreeMap<Box<str>, LocaleText>;

/// Spell list locale map: keys are spell names.
pub type SpellLocaleMap = BTreeMap<Box<str>, LocaleText>;

/// Apply locale to an `Index`.
pub(super) fn apply_index_locale(index: &mut Index, locale: &IndexLocaleMap) {
    for (key, text) in locale {
        let (prefix, name) = match key.find('.') {
            Some(pos) => (&key[..pos], &key[pos + 1..]),
            None => continue,
        };
        match prefix {
            "class" => {
                if let Some(entry) = index.classes.get_mut(name) {
                    text.apply_label(&mut entry.label);
                    text.apply_description(&mut entry.description);
                }
            }
            "race" => {
                if let Some(entry) = index.races.get_mut(name) {
                    text.apply_label(&mut entry.label);
                    text.apply_description(&mut entry.description);
                }
            }
            "background" => {
                if let Some(entry) = index.backgrounds.get_mut(name) {
                    text.apply_label(&mut entry.label);
                    text.apply_description(&mut entry.description);
                }
            }
            "spell" => {
                if let Some(entry) = index.spells.get_mut(name) {
                    text.apply_label(&mut entry.label);
                }
            }
            _ => {}
        }
    }
}

/// Apply locale to an `EffectsIndex`.
pub fn apply_effects_locale(effects: &mut EffectsIndex, locale: &EffectsLocaleMap) {
    for (name, text) in locale {
        if let Some(effect) = effects.0.get_mut(name.as_ref()) {
            text.apply_description(&mut effect.description);
        }
    }
}

/// Apply locale to a `FeaturesIndex`.
/// Keys are flat: `"Rage"` for label/description, `"Rage.field.X"` for
/// sub-paths.
pub fn apply_features_locale(features: &mut FeaturesIndex, locale: &LocaleMap) {
    // First pass: feature-level label/description (keys without dots = bare feature
    // names)
    for (key, text) in locale {
        if !key.is_bare() {
            continue;
        }
        if let Some(feat) = features.0.get_mut(key.as_str()) {
            text.apply_label(&mut feat.label);
            text.apply_description(&mut feat.description);
        }
    }
    // Second pass: field/option/spell sub-keys
    let feature_names: Vec<Box<str>> = features.0.keys().cloned().collect();
    for feat_name in &feature_names {
        if let Some(feat) = features.0.get_mut(feat_name.as_ref()) {
            apply_locale_to_feature(
                feat,
                feat_name,
                locale,
                LocaleKey::flat_field,
                LocaleKey::flat_field_option,
                LocaleKey::flat_spell,
            );
        }
    }
}

/// Apply locale to a `SpellMap`.
pub fn apply_spell_map_locale(spells: &mut SpellMap, locale: &SpellLocaleMap) {
    for (name, text) in locale {
        if let Some(spell_def) = spells.0.get_mut(name.as_ref()) {
            text.apply_label(&mut spell_def.label);
            text.apply_description(&mut spell_def.description);
        }
    }
}
