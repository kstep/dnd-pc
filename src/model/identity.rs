use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::model::Alignment;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct CharacterIdentity {
    #[serde(default)]
    pub classes: Vec<ClassLevel>,
    #[serde(default, alias = "race")]
    pub species: String,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub experience_points: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct ClassLevel {
    #[serde(default)]
    pub class: Box<str>,
    #[serde(default)]
    pub class_label: Option<String>,
    #[serde(default)]
    pub subclass: Option<Box<str>>,
    #[serde(default)]
    pub subclass_label: Option<String>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub hit_die_sides: u32,
    #[serde(default)]
    pub hit_dice_used: u32,
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
            class: Box::default(),
            class_label: None,
            subclass: None,
            subclass_label: None,
            level: 1,
            hit_die_sides: 8,
            hit_dice_used: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct Personality {
    #[serde(default = "default_character_name")]
    pub name: String,
    pub alignment: Alignment,
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

fn default_character_name() -> String {
    "New Character".to_string()
}

impl Default for Personality {
    fn default() -> Self {
        Self {
            name: default_character_name(),
            alignment: Alignment::TrueNeutral,
            history: String::new(),
            personality_traits: String::new(),
            ideals: String::new(),
            bonds: String::new(),
            flaws: String::new(),
        }
    }
}
