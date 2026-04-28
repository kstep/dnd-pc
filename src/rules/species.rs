use serde::Deserialize;

use crate::{demap::Named, vecset::VecSet};

#[derive(Debug, Clone, Deserialize)]
pub struct SpeciesDefinition {
    pub name: Box<str>,
    #[serde(default)]
    pub features: VecSet<String>,
}

impl Named for SpeciesDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}
