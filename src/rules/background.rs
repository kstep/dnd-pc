use serde::Deserialize;

use crate::{demap::Named, rules::packages::HasPackage, vecset::VecSet};

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundDefinition {
    /// Source package id, stamped during merge; empty = unknown.
    #[serde(skip)]
    pub package: Box<str>,
    pub name: Box<str>,
    #[serde(default)]
    pub features: VecSet<String>,
}

impl Named for BackgroundDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl HasPackage for BackgroundDefinition {
    fn set_package(&mut self, package: &str) {
        self.package = package.into();
    }
}
