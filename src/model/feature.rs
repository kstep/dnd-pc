use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::model::{Die, SpellData};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Feature {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
}

impl Feature {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeatureSource {
    Class(String),
    Race(String),
    Background(String),
}

impl FeatureSource {
    pub fn name(&self) -> &str {
        match self {
            Self::Class(name) | Self::Race(name) | Self::Background(name) => name,
        }
    }

    pub fn as_class(&self) -> Option<&str> {
        match self {
            Self::Class(name) => Some(name),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct FeatureData {
    #[serde(default)]
    pub source: Option<FeatureSource>,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct RacialTrait {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
}

impl RacialTrait {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}
