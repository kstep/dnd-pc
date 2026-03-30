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

    /// Find the class level at which a feature first appears (checking both
    /// base class and subclass level tables). Returns 0 if not found.
    pub fn feature_level(&self, subclass: Option<&str>, feature_name: &str) -> u32 {
        for (index, level_rules) in self.levels.iter().enumerate() {
            if level_rules.features.iter().any(|name| name == feature_name) {
                return index as u32 + 1;
            }
        }
        if let Some(subclass_def) = subclass.and_then(|sc| self.subclasses.get(sc)) {
            for (level, level_rules) in subclass_def.levels.iter() {
                if level_rules.features.iter().any(|name| name == feature_name) {
                    return *level;
                }
            }
        }
        0
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
