use std::collections::{BTreeMap, HashSet};

use serde::Deserialize;

use crate::{
    demap::{self, Named},
    rules::utils::LevelRules,
    vecset::VecSet,
};

#[derive(Debug, Clone, Deserialize)]
pub struct ClassDefinition {
    pub name: Box<str>,
    pub hit_die: u32,
    #[serde(default)]
    pub levels: LevelRules<ClassLevelRules>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub subclasses: BTreeMap<Box<str>, SubclassDefinition>,
}

impl Named for ClassDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl ClassDefinition {
    /// Maximum class level declared in the progression table. Falls back to
    /// the D&D 5e standard cap of 20 for classes that haven't been fetched
    /// yet or provide no explicit level rules.
    pub fn max_level(&self) -> u32 {
        self.levels.keys().next_back().copied().unwrap_or(20)
    }

    /// Find the class level at which a feature first appears (checking both
    /// base class and subclass level tables). Returns 0 if not found.
    pub fn feature_level(&self, subclass: Option<&str>, feature_name: &str) -> u32 {
        for (level, level_rules) in self.levels.iter() {
            if level_rules.features.iter().any(|name| name == feature_name) {
                return *level;
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

    /// Iterate unique feature names from class levels and subclass levels, in
    /// first-occurrence order. Features that appear on multiple levels (e.g.
    /// Sorcerer's Metamagic at 2/10/17) are yielded once.
    pub fn feature_names<'a>(&'a self, subclass: Option<&str>) -> impl Iterator<Item = &'a str> {
        let sc_features = subclass
            .and_then(|s| self.subclasses.get(s))
            .into_iter()
            .flat_map(|sc| sc.levels.values())
            .flat_map(|lr| lr.features.iter().map(String::as_str));
        let mut seen = HashSet::new();
        self.levels
            .values()
            .flat_map(|lr| lr.features.iter().map(String::as_str))
            .chain(sc_features)
            .filter(move |name| seen.insert(*name))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassDefinition {
    pub name: Box<str>,
    #[serde(default)]
    pub levels: LevelRules<ClassLevelRules>,
}

impl Named for SubclassDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl SubclassDefinition {
    pub fn min_level(&self) -> u32 {
        self.levels.keys().next().copied().unwrap_or(1)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClassLevelRules {
    #[serde(default)]
    pub features: VecSet<String>,
}
