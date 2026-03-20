use std::{collections::BTreeMap, fmt};

use serde::Deserialize;

use super::{
    background::BackgroundDefinition,
    class::ClassDefinition,
    feature::{ChoiceOptions, FeaturesIndex, FieldKind},
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

impl LocaleKey {
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
}

// --- Application to definition types ---

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
            // Field labels/descriptions
            for (field_name, field_def) in &mut feat.fields {
                let field_key = LocaleKey::flat_field(feat_name, field_name);
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
                        let opt_key =
                            LocaleKey::flat_field_option(feat_name, field_name, &opt.name);
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
                    let spell_key = LocaleKey::flat_spell(feat_name, spell_name);
                    if let Some(text) = locale.get(&spell_key) {
                        text.apply_label(&mut spell_def.label);
                        text.apply_description(&mut spell_def.description);
                    }
                }
            }
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
