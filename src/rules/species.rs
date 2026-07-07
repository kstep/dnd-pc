use serde::Deserialize;

use crate::{demap::Named, rules::packages::HasPackage, vecset::VecSet};

#[derive(Debug, Clone, Deserialize)]
pub struct SpeciesDefinition {
    /// Source package id, stamped during merge; empty = unknown.
    #[serde(skip)]
    pub package: Box<str>,
    pub name: Box<str>,
    #[serde(default)]
    pub features: VecSet<String>,
}

impl Named for SpeciesDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl HasPackage for SpeciesDefinition {
    fn set_package(&mut self, package: &str) {
        self.package = package.into();
    }
}
