use std::collections::BTreeMap;

use serde::Deserialize;

use super::{feature::FeatureDefinition, race::AbilityModifier};
use crate::{
    demap,
    model::{Character, FeatureSource, ProficiencyLevel, Skill},
    vecset::VecSet,
};

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub description: String,
    #[serde(default)]
    pub ability_modifiers: Vec<AbilityModifier>,
    #[serde(default)]
    pub proficiencies: VecSet<Skill>,
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

            for am in &self.ability_modifiers {
                character.modify_ability(am.ability, am.modifier);
            }

            character.update_skill_proficiencies(|skills| {
                for &skill in &self.proficiencies {
                    skills.entry(skill).or_insert(ProficiencyLevel::Proficient);
                }
            });
        }

        let source = FeatureSource::Background(self.name.clone());
        for feat in self.features.values() {
            feat.apply(character.level(), character, &source);
        }
    }
}
