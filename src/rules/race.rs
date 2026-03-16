use std::collections::BTreeMap;

use serde::Deserialize;

use super::feature::{Assignment, FeatureDefinition, WhenCondition};
use crate::{
    demap::{self, Named},
    model::{Character, FeatureSource, RacialTrait},
    vecset::VecSet,
};

#[derive(Debug, Clone, Deserialize)]
pub struct RaceTrait {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub description: String,
    #[serde(default)]
    pub languages: VecSet<String>,
    #[serde(default)]
    pub assign: Option<Vec<Assignment>>,
}

impl RaceTrait {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn assign(&self, character: &mut Character, when: WhenCondition) {
        let Some(assignments) = &self.assign else {
            return;
        };
        for a in assignments.iter().filter(|a| a.when == when) {
            if let Err(error) = a.expr.apply(character) {
                log::error!(
                    "Failed to apply racial trait assignment for '{}': {error:?}",
                    self.name,
                );
            }
        }
    }
}

impl Named for RaceTrait {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RaceDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub description: String,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub traits: BTreeMap<Box<str>, RaceTrait>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub features: BTreeMap<Box<str>, FeatureDefinition>,
}

impl RaceDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn apply(&self, character: &mut Character) {
        if !character.identity.race_applied {
            character.identity.race_applied = true;

            for racial_trait in self.traits.values() {
                character.racial_traits.push(RacialTrait {
                    name: racial_trait.name.clone(),
                    label: racial_trait.label.clone(),
                    description: racial_trait.description.clone(),
                });
                character
                    .languages
                    .extend(racial_trait.languages.iter().cloned());
                racial_trait.assign(character, WhenCondition::OnFeatureAdd);
            }
        }

        let total_level = character.level();
        let source = FeatureSource::Race(self.name.clone());
        for feat in self.features.values() {
            feat.apply(total_level, character, &source);
        }
    }
}
