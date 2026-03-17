use std::{collections::BTreeMap, fmt};

use serde::Deserialize;

use super::{
    background::BackgroundDefinition,
    class::ClassDefinition,
    feature::{ChoiceOptions, FeatureDefinition, FieldKind},
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
            "trait" => match rest {
                Some(name) => LocalePath::Trait(name),
                None => LocalePath::Unknown,
            },
            _ => LocalePath::Unknown,
        }
    }
}

/// Split `s` at the first `.` that separates a keyword from the rest.
/// For keywords like "feature", "subclass", "field", "option", "spell",
/// "trait", the split is at the first dot after the keyword.
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
    /// `"trait.X"` (for races)
    Trait(&'a str),
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
    #[serde(default)]
    pub short: Option<String>,
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

    pub fn with_short(mut self, short: impl Into<String>) -> Self {
        self.short = Some(short.into());
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

    /// Always assign short (clears to None if locale doesn't have one).
    pub fn apply_short(&self, target: &mut Option<String>) {
        *target = self.short.clone();
    }

    /// Returns true if all fields are None/empty.
    pub fn is_empty(&self) -> bool {
        self.label.is_none() && self.description.is_none() && self.short.is_none()
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
        use serde::ser::SerializeMap;
        // Count non-None fields
        let count = self.label.is_some() as usize
            + self.description.is_some() as usize
            + self.short.is_some() as usize;
        let mut map = serializer.serialize_map(Some(count))?;
        if let Some(label) = &self.label {
            map.serialize_entry("label", label)?;
        }
        if let Some(desc) = &self.description {
            map.serialize_entry("description", desc)?;
        }
        if let Some(short) = &self.short {
            map.serialize_entry("short", short)?;
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

    pub fn race_trait(name: &str) -> Self {
        Self(format!("trait.{name}").into_boxed_str())
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
            // Apply short to Points fields
            if let FieldKind::Points { short, .. } = &mut field_def.kind {
                text.apply_short(short);
            }
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
pub fn apply_class_locale(def: &mut ClassDefinition, locale: &LocaleMap) {
    for (key, text) in locale {
        match key.parse() {
            LocalePath::Root => {
                text.apply_label(&mut def.label);
                text.apply_description(&mut def.description);
            }
            LocalePath::Feature(name) => {
                if let Some(feat) = def.features.get_mut(name) {
                    text.apply_label(&mut feat.label);
                    text.apply_description(&mut feat.description);
                }
            }
            LocalePath::Subclass(name) => {
                if let Some(sc) = def.subclasses.get_mut(name) {
                    text.apply_label(&mut sc.label);
                    text.apply_description(&mut sc.description);
                }
            }
            LocalePath::SubclassFeature(sc_name, feat_name) => {
                if let Some(sc) = def.subclasses.get_mut(sc_name)
                    && let Some(feat) = sc.features.get_mut(feat_name)
                {
                    text.apply_label(&mut feat.label);
                    text.apply_description(&mut feat.description);
                }
            }
            // Field/option/spell handled below via second pass
            _ => {}
        }
    }

    // Second pass: apply nested feature content (fields, options, spells)
    let feature_names: Vec<Box<str>> = def.features.keys().cloned().collect();
    for feat_name in &feature_names {
        if let Some(feat) = def.features.get_mut(feat_name.as_ref()) {
            apply_locale_to_feature(
                feat,
                feat_name,
                locale,
                LocaleKey::feature_field,
                LocaleKey::feature_field_option,
                LocaleKey::feature_spell,
            );
        }
    }

    // Subclass features second pass
    let sc_names: Vec<Box<str>> = def.subclasses.keys().cloned().collect();
    for sc_name in &sc_names {
        if let Some(sc) = def.subclasses.get_mut(sc_name.as_ref()) {
            let sc_feat_names: Vec<Box<str>> = sc.features.keys().cloned().collect();
            for feat_name in &sc_feat_names {
                if let Some(feat) = sc.features.get_mut(feat_name.as_ref()) {
                    let sc_n = sc_name.clone();
                    apply_locale_to_feature(
                        feat,
                        feat_name,
                        locale,
                        |f, field| LocaleKey::subclass_feature_field(&sc_n, f, field),
                        |f, field, opt| {
                            LocaleKey::subclass_feature_field_option(&sc_n, f, field, opt)
                        },
                        // Subclass feature spells are unusual but handle for completeness
                        |f, spell| {
                            // No dedicated constructor — use feature_spell as these are rare
                            LocaleKey(
                                format!("subclass.{sc_n}.feature.{f}.spell.{spell}")
                                    .into_boxed_str(),
                            )
                        },
                    );
                }
            }
        }
    }
}

/// Apply a locale map to a `RaceDefinition`.
pub fn apply_race_locale(def: &mut RaceDefinition, locale: &LocaleMap) {
    for (key, text) in locale {
        match key.parse() {
            LocalePath::Root => {
                text.apply_label(&mut def.label);
                text.apply_description(&mut def.description);
            }
            LocalePath::Trait(name) => {
                if let Some(rt) = def.traits.get_mut(name) {
                    text.apply_label(&mut rt.label);
                    text.apply_description(&mut rt.description);
                }
            }
            LocalePath::Feature(name) => {
                if let Some(feat) = def.features.get_mut(name) {
                    text.apply_label(&mut feat.label);
                    text.apply_description(&mut feat.description);
                }
            }
            _ => {}
        }
    }

    // Feature nested content
    let feature_names: Vec<Box<str>> = def.features.keys().cloned().collect();
    for feat_name in &feature_names {
        if let Some(feat) = def.features.get_mut(feat_name.as_ref()) {
            apply_locale_to_feature(
                feat,
                feat_name,
                locale,
                LocaleKey::feature_field,
                LocaleKey::feature_field_option,
                LocaleKey::feature_spell,
            );
        }
    }
}

/// Apply a locale map to a `BackgroundDefinition`.
pub fn apply_background_locale(def: &mut BackgroundDefinition, locale: &LocaleMap) {
    for (key, text) in locale {
        match key.parse() {
            LocalePath::Root => {
                text.apply_label(&mut def.label);
                text.apply_description(&mut def.description);
            }
            LocalePath::Feature(name) => {
                if let Some(feat) = def.features.get_mut(name) {
                    text.apply_label(&mut feat.label);
                    text.apply_description(&mut feat.description);
                }
            }
            _ => {}
        }
    }

    // Feature nested content
    let feature_names: Vec<Box<str>> = def.features.keys().cloned().collect();
    for feat_name in &feature_names {
        if let Some(feat) = def.features.get_mut(feat_name.as_ref()) {
            apply_locale_to_feature(
                feat,
                feat_name,
                locale,
                LocaleKey::feature_field,
                LocaleKey::feature_field_option,
                LocaleKey::feature_spell,
            );
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

    /// Extract label, description, and short from a Points field.
    pub fn extract_with_short(
        label: &mut Option<String>,
        description: &mut String,
        short: &mut Option<String>,
    ) -> Self {
        let mut text = Self::extract(label, description);
        text.short = short.take();
        text
    }
}

/// Extract locale text from a `ClassDefinition`, returning the locale map
/// and leaving the definition with empty text fields.
pub fn extract_class_locale(def: &mut ClassDefinition) -> LocaleMap {
    let mut map = LocaleMap::new();

    // Root
    let root = LocaleText::extract(&mut def.label, &mut def.description);
    if !root.is_empty() {
        map.insert(LocaleKey::root(), root);
    }

    // Features
    for (feat_name, feat) in &mut def.features {
        extract_feature_locale(
            feat,
            feat_name,
            &mut map,
            LocaleKey::feature_field,
            LocaleKey::feature_field_option,
            LocaleKey::feature_spell,
            LocaleKey::feature,
        );
    }

    // Subclasses
    for (sc_name, sc) in &mut def.subclasses {
        let sc_text = LocaleText::extract(&mut sc.label, &mut sc.description);
        if !sc_text.is_empty() {
            map.insert(LocaleKey::subclass(sc_name), sc_text);
        }

        for (feat_name, feat) in &mut sc.features {
            let sc_n = sc_name.clone();
            extract_feature_locale(
                feat,
                feat_name,
                &mut map,
                |f, field| LocaleKey::subclass_feature_field(&sc_n, f, field),
                |f, field, opt| LocaleKey::subclass_feature_field_option(&sc_n, f, field, opt),
                |f, spell| {
                    LocaleKey(format!("subclass.{sc_n}.feature.{f}.spell.{spell}").into_boxed_str())
                },
                |f| LocaleKey::subclass_feature(&sc_n, f),
            );
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

    for (trait_name, rt) in &mut def.traits {
        let text = LocaleText::extract(&mut rt.label, &mut rt.description);
        if !text.is_empty() {
            map.insert(LocaleKey::race_trait(trait_name), text);
        }
    }

    for (feat_name, feat) in &mut def.features {
        extract_feature_locale(
            feat,
            feat_name,
            &mut map,
            LocaleKey::feature_field,
            LocaleKey::feature_field_option,
            LocaleKey::feature_spell,
            LocaleKey::feature,
        );
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

    for (feat_name, feat) in &mut def.features {
        extract_feature_locale(
            feat,
            feat_name,
            &mut map,
            LocaleKey::feature_field,
            LocaleKey::feature_field_option,
            LocaleKey::feature_spell,
            LocaleKey::feature,
        );
    }

    map
}

/// Extract locale text from a single feature into the map.
fn extract_feature_locale(
    feat: &mut FeatureDefinition,
    feat_name: &str,
    map: &mut LocaleMap,
    make_field_key: impl Fn(&str, &str) -> LocaleKey,
    make_option_key: impl Fn(&str, &str, &str) -> LocaleKey,
    make_spell_key: impl Fn(&str, &str) -> LocaleKey,
    make_feature_key: impl Fn(&str) -> LocaleKey,
) {
    let text = LocaleText::extract(&mut feat.label, &mut feat.description);
    if !text.is_empty() {
        map.insert(make_feature_key(feat_name), text);
    }

    for (field_name, field_def) in &mut feat.fields {
        let field_text = if let FieldKind::Points { short, .. } = &mut field_def.kind {
            LocaleText::extract_with_short(&mut field_def.label, &mut field_def.description, short)
        } else {
            LocaleText::extract(&mut field_def.label, &mut field_def.description)
        };
        if !field_text.is_empty() {
            map.insert(make_field_key(feat_name, field_name), field_text);
        }

        if let FieldKind::Choice {
            options: ChoiceOptions::List(opts),
            ..
        } = &mut field_def.kind
        {
            for opt in opts {
                let opt_text = LocaleText::extract(&mut opt.label, &mut opt.description);
                if !opt_text.is_empty() {
                    map.insert(make_option_key(feat_name, field_name, &opt.name), opt_text);
                }
            }
        }
    }

    if let Some(spells_def) = &mut feat.spells
        && let super::spells::SpellList::Inline(spell_map) = &mut spells_def.list
    {
        for (spell_name, spell_def) in spell_map.0.iter_mut() {
            let spell_text = LocaleText::extract(&mut spell_def.label, &mut spell_def.description);
            if !spell_text.is_empty() {
                map.insert(make_spell_key(feat_name, spell_name), spell_text);
            }
        }
    }
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

/// Apply locale to a `SpellMap`.
pub fn apply_spell_map_locale(spells: &mut SpellMap, locale: &SpellLocaleMap) {
    for (name, text) in locale {
        if let Some(spell_def) = spells.0.get_mut(name.as_ref()) {
            text.apply_label(&mut spell_def.label);
            text.apply_description(&mut spell_def.description);
        }
    }
}
