use serde::Deserialize;

use crate::vecset::VecSet;

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub features: VecSet<String>,
}

impl BackgroundDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}
