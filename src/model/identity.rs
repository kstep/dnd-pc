use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::{model::Alignment, vecset::VecSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct CharacterIdentity {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub classes: Vec<ClassLevel>,
    #[serde(default)]
    pub race: String,
    #[serde(default)]
    pub background: String,
    pub alignment: Alignment,
    #[serde(default)]
    pub experience_points: u32,
    #[serde(default)]
    pub race_applied: bool,
    #[serde(default)]
    pub background_applied: bool,
}

impl Default for CharacterIdentity {
    fn default() -> Self {
        Self {
            name: "New Character".to_string(),
            classes: vec![ClassLevel::default()],
            race: String::new(),
            background: String::new(),
            alignment: Alignment::TrueNeutral,
            experience_points: 0,
            race_applied: false,
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
