use std::collections::BTreeMap;

use serde::Deserialize;

use super::utils::LevelRules;
use crate::{
    demap::{self, Named},
    vecset::VecSet,
};

#[derive(Debug, Clone, Deserialize)]
pub struct ClassDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    pub hit_die: u32,
    #[serde(default)]
    pub levels: Vec<ClassLevelRules>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub subclasses: BTreeMap<Box<str>, SubclassDefinition>,
}

impl ClassDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    /// Iterate all feature names from class levels and subclass levels.
    pub fn feature_names<'a>(&'a self, subclass: Option<&str>) -> impl Iterator<Item = &'a str> {
        let sc_features = subclass
            .and_then(|s| self.subclasses.get(s))
            .into_iter()
            .flat_map(|sc| sc.levels.values())
            .flat_map(|lr| lr.features.iter().map(String::as_str));
        self.levels
            .iter()
            .flat_map(|lr| lr.features.iter().map(String::as_str))
            .chain(sc_features)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub levels: LevelRules<SubclassLevelRules>,
}

impl Named for SubclassDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl SubclassDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn min_level(&self) -> u32 {
        self.levels.keys().next().copied().unwrap_or(1)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassLevelRules {
    pub level: u32,
    #[serde(default)]
    pub features: VecSet<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClassLevelRules {
    #[serde(default)]
    pub features: VecSet<String>,
}
