use std::collections::BTreeMap;

use serde::Deserialize;

use super::{feature::FeatureDefinition, utils::LevelRules};
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
    #[serde(default, deserialize_with = "demap::named_map")]
    pub features: BTreeMap<Box<str>, FeatureDefinition>,
    #[serde(default)]
    pub levels: Vec<ClassLevelRules>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub subclasses: BTreeMap<Box<str>, SubclassDefinition>,
}

impl ClassDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn features(&self, subclass: Option<&str>) -> impl Iterator<Item = &FeatureDefinition> {
        let sc_features = subclass
            .and_then(|name| self.subclasses.get(name))
            .into_iter()
            .flat_map(|sc| sc.features.values());
        self.features.values().chain(sc_features)
    }

    pub fn find_feature(&self, name: &str, subclass: Option<&str>) -> Option<&FeatureDefinition> {
        self.features.get(name).or_else(|| {
            subclass
                .and_then(|sc| self.subclasses.get(sc))
                .and_then(|sc| sc.features.get(name))
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub features: BTreeMap<Box<str>, FeatureDefinition>,
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
