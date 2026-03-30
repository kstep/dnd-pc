use std::fmt;

use leptos_fluent::I18n;
use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::{
    expr::DicePool,
    model::{Die, SpellData},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Feature {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub applied: bool,
    #[serde(default)]
    pub source: FeatureSource,
}

impl Feature {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn set_label(&mut self, value: String) {
        self.label = Some(value);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeatureSource {
    Class(String, u32),
    #[serde(alias = "Race")]
    Species(String),
    Background(String),
    User(u32),
}

impl Default for FeatureSource {
    fn default() -> Self {
        Self::User(0)
    }
}

impl fmt::Display for FeatureSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Class(name, level) => write!(f, "Class: {name} ({level})"),
            Self::Species(name) => write!(f, "Species: {name}"),
            Self::Background(name) => write!(f, "Background: {name}"),
            Self::User(level) => write!(f, "User ({level})"),
        }
    }
}

impl FeatureSource {
    pub fn display_name(&self, i18n: I18n) -> Option<String> {
        match self {
            Self::Class(name, level) => {
                let prefix = i18n.tr("source-class");
                Some(format!("{prefix}: {name} ({level})"))
            }
            Self::Species(name) => {
                let prefix = i18n.tr("source-species");
                Some(format!("{prefix}: {name}"))
            }
            Self::Background(name) => {
                let prefix = i18n.tr("source-background");
                Some(format!("{prefix}: {name}"))
            }
            Self::User(_) => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Class(name, _) | Self::Species(name) | Self::Background(name) => name,
            Self::User(_) => "",
        }
    }

    pub fn as_class(&self) -> Option<&str> {
        match self {
            Self::Class(name, _) => Some(name),
            _ => None,
        }
    }

    pub fn added_at_level(&self) -> u32 {
        match self {
            Self::Class(_, level) | Self::User(level) => *level,
            Self::Species(_) | Self::Background(_) => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct AssignInputs {
    #[serde(default)]
    pub args: Vec<i32>,
    #[serde(default)]
    pub dice: DicePool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct FeatureData {
    #[serde(default)]
    pub inputs: Vec<AssignInputs>,
    #[serde(default)]
    pub fields: Vec<FeatureField>,
    #[serde(default)]
    pub spells: Option<SpellData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct FeatureField {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub value: FeatureValue,
}

impl FeatureField {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub enum FeatureValue {
    Points { used: u32, max: u32 },
    Choice { options: Vec<FeatureOption> },
    Die { die: Die, used: u32 },
    Bonus(i32),
}

impl Default for FeatureValue {
    fn default() -> Self {
        FeatureValue::Points { used: 0, max: 0 }
    }
}

/// Derive a short abbreviation from a name by taking the first letter of each
/// word. "Channel Divinity" → "CD", "Sorcery Points" → "SP", "Rages" → "R"
pub fn short_name(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .collect()
}

impl FeatureValue {
    pub fn available_points(&self) -> Option<u32> {
        match self {
            FeatureValue::Points { used, max } => Some(max.saturating_sub(*used)),
            FeatureValue::Die { die, used } => Some(die.amount.saturating_sub(*used)),
            _ => None,
        }
    }

    pub fn choices(&self) -> &[FeatureOption] {
        match self {
            FeatureValue::Choice { options } => options,
            _ => &[],
        }
    }

    pub fn choices_mut(&mut self) -> &mut [FeatureOption] {
        match self {
            FeatureValue::Choice { options } => options,
            _ => &mut [],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct FeatureOption {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cost: u32,
}

impl FeatureOption {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn set_label(&mut self, value: String) {
        self.label = Some(value);
    }
}
