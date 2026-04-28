use serde::Deserialize;

use crate::{demap::Named, vecset::VecSet};

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundDefinition {
    pub name: Box<str>,
    #[serde(default)]
    pub features: VecSet<String>,
}

impl Named for BackgroundDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}
