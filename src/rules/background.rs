use std::collections::BTreeMap;

use serde::Deserialize;

use super::feature::FeatureDefinition;
use crate::{
    demap,
    model::{Character, FeatureSource},
};

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub features: BTreeMap<Box<str>, FeatureDefinition>,
}

impl BackgroundDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn apply(&self, character: &mut Character) {
        if !character.identity.background_applied {
            character.identity.background_applied = true;
        }

        let source = FeatureSource::Background(self.name.clone());
        for feat in self.features.values() {
            feat.apply(character.level(), character, &source);
        }
    }
}
