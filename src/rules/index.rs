use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    demap::{self, Named},
    expr::{Eval as _, Expr},
    model::{Attribute, Character},
};

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
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub prerequisites: Option<Expr<Attribute>>,
}

impl Named for ClassIndexEntry {
    fn name(&self) -> &str {
        &self.name
    }
}

impl ClassIndexEntry {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn meets_prerequisites(&self, character: &Character) -> bool {
        self.prerequisites
            .as_ref()
            .is_none_or(|expr| expr.eval(character).unwrap_or(0) != 0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpeciesIndexEntry {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub url: String,
    #[serde(default)]
    pub description: String,
}

impl Named for SpeciesIndexEntry {
    fn name(&self) -> &str {
        &self.name
    }
}

impl SpeciesIndexEntry {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundIndexEntry {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub url: String,
    #[serde(default)]
    pub description: String,
}

impl Named for BackgroundIndexEntry {
    fn name(&self) -> &str {
        &self.name
    }
}

impl BackgroundIndexEntry {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpellIndexEntry {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub url: String,
}

impl Named for SpellIndexEntry {
    fn name(&self) -> &str {
        &self.name
    }
}

impl SpellIndexEntry {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}
