use std::collections::BTreeMap;

use serde::Deserialize;
use strum::Display;

use crate::{
    demap::{self, Named},
    expr::Eval as _,
    model::{Character, Expr},
};

/// Borrowed reference into one of the four entry types on the shared
/// `Index` (class / species / background / spell). The `Display` impl
/// produces the locale-overlay key (`"class.wizard"`, `"species.elf"`,
/// …) used by `LocalizedIndex<Index, IndexLocaleMap>::entry_label_desc`.
#[derive(Copy, Clone, Display)]
pub enum IndexEntry<'a> {
    #[strum(to_string = "class.{0}")]
    Class(&'a str),
    #[strum(to_string = "species.{0}")]
    Species(&'a str),
    #[strum(to_string = "background.{0}")]
    Background(&'a str),
    #[strum(to_string = "spell.{0}")]
    Spell(&'a str),
}

impl<'a> IndexEntry<'a> {
    pub fn name(&self) -> &'a str {
        match *self {
            Self::Class(n) | Self::Species(n) | Self::Background(n) | Self::Spell(n) => n,
        }
    }

    /// Path prefix (`"class"` / `"species"` / …) for `/r/<prefix>/<name>`
    /// routes.
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Class(_) => "class",
            Self::Species(_) => "species",
            Self::Background(_) => "background",
            Self::Spell(_) => "spell",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Index {
    #[serde(deserialize_with = "demap::named_map")]
    pub classes: BTreeMap<Box<str>, ClassIndexEntry>,
    #[serde(default, alias = "races", deserialize_with = "demap::named_map")]
    pub species: BTreeMap<Box<str>, SpeciesIndexEntry>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub backgrounds: BTreeMap<Box<str>, BackgroundIndexEntry>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub spells: BTreeMap<Box<str>, SpellIndexEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassIndexEntry {
    pub name: Box<str>,
    pub url: Box<str>,
    #[serde(default)]
    pub prerequisites: Option<Expr>,
}

impl Named for ClassIndexEntry {
    fn name(&self) -> &str {
        &self.name
    }
}

impl ClassIndexEntry {
    pub fn meets_prerequisites(&self, character: &Character) -> bool {
        self.prerequisites
            .as_ref()
            .is_none_or(|expr| expr.eval(character).unwrap_or(0) != 0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpeciesIndexEntry {
    pub name: Box<str>,
    pub url: Box<str>,
}

impl Named for SpeciesIndexEntry {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundIndexEntry {
    pub name: Box<str>,
    pub url: Box<str>,
}

impl Named for BackgroundIndexEntry {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpellIndexEntry {
    pub name: Box<str>,
    pub url: Box<str>,
}

impl Named for SpellIndexEntry {
    fn name(&self) -> &str {
        &self.name
    }
}
