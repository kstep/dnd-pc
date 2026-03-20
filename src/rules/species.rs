use serde::Deserialize;

use crate::vecset::VecSet;

#[derive(Debug, Clone, Deserialize)]
pub struct SpeciesDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub features: VecSet<String>,
}

impl SpeciesDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}
