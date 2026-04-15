use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::{model::Alignment, vecset::VecSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct CharacterIdentity {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub classes: Vec<ClassLevel>,
    #[serde(default, alias = "race")]
    pub species: String,
    #[serde(default)]
    pub background: String,
    pub alignment: Alignment,
    #[serde(default)]
    pub experience_points: u32,
    #[serde(default, alias = "race_applied")]
    pub species_applied: bool,
    #[serde(default)]
    pub background_applied: bool,
}

impl CharacterIdentity {
    pub fn reset_applied_flags(&mut self) {
        self.species_applied = false;
        self.background_applied = false;
        for class_level in &mut self.classes {
            class_level.applied_levels.clear();
        }
    }
}

impl Default for CharacterIdentity {
    fn default() -> Self {
        Self {
            name: "New Character".to_string(),
            classes: vec![ClassLevel::default()],
            species: String::new(),
            background: String::new(),
            alignment: Alignment::TrueNeutral,
            experience_points: 0,
            species_applied: false,
            background_applied: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct ClassLevel {
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub class_label: Option<String>,
    #[serde(default)]
    pub subclass: Option<String>,
    #[serde(default)]
    pub subclass_label: Option<String>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub hit_die_sides: u32,
    #[serde(default)]
    pub hit_dice_used: u32,
    #[serde(default)]
    pub applied_levels: VecSet<u32>,
}

impl ClassLevel {
    pub fn class_label(&self) -> &str {
        self.class_label.as_deref().unwrap_or(&self.class)
    }

    pub fn subclass_label(&self) -> Option<&str> {
        self.subclass_label.as_deref().or(self.subclass.as_deref())
    }
}

impl std::fmt::Display for ClassLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(subclass) = self.subclass_label() {
            write!(f, "{} ({}) {}", self.class_label(), subclass, self.level)
        } else {
            write!(f, "{} {}", self.class_label(), self.level)
        }
    }
}

/// Format a list of classes into a human-readable string like
/// `"Fighter (Champion) 5 / Rogue 3"`. Empty classes are skipped.
pub fn format_classes(classes: &[ClassLevel]) -> String {
    classes
        .iter()
        .filter(|c| !c.class.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" / ")
}

impl Default for ClassLevel {
    fn default() -> Self {
        Self {
            class: String::new(),
            class_label: None,
            subclass: None,
            subclass_label: None,
            level: 1,
            hit_die_sides: 8,
            hit_dice_used: 0,
            applied_levels: VecSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;

    #[wasm_bindgen_test]
    fn reset_applied_flags_clears_everything() {
        let mut identity = CharacterIdentity {
            species_applied: true,
            background_applied: true,
            classes: vec![
                ClassLevel {
                    class: "Monk".into(),
                    level: 3,
                    applied_levels: [1, 2, 3].into_iter().collect(),
                    ..ClassLevel::default()
                },
                ClassLevel {
                    class: "Wizard".into(),
                    level: 2,
                    applied_levels: [1, 2].into_iter().collect(),
                    ..ClassLevel::default()
                },
            ],
            ..CharacterIdentity::default()
        };

        identity.reset_applied_flags();

        assert!(!identity.species_applied);
        assert!(!identity.background_applied);
        for cl in &identity.classes {
            assert!(
                cl.applied_levels.is_empty(),
                "{} still has applied levels",
                cl.class
            );
        }
        // Levels themselves are preserved.
        assert_eq!(identity.classes[0].level, 3);
        assert_eq!(identity.classes[1].level, 2);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Personality {
    #[serde(default)]
    pub history: String,
    #[serde(default)]
    pub personality_traits: String,
    #[serde(default)]
    pub ideals: String,
    #[serde(default)]
    pub bonds: String,
    #[serde(default)]
    pub flaws: String,
}
