use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::model::{Ability, SpellSlotPool};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct SpellData {
    pub casting_ability: Ability,
    #[serde(default)]
    pub caster_coef: u32,
    #[serde(default)]
    pub pool: SpellSlotPool,
    #[serde(default)]
    pub spells: Vec<Spell>,
}

impl SpellData {
    pub fn cantrips(&self) -> impl Iterator<Item = &Spell> {
        self.spells.iter().filter(|s| s.level == 0)
    }

    pub fn spells(&self) -> impl Iterator<Item = &Spell> {
        self.spells.iter().filter(|s| s.level > 0)
    }
}

impl Default for SpellData {
    fn default() -> Self {
        Self {
            casting_ability: Ability::Intelligence,
            caster_coef: 0,
            pool: SpellSlotPool::default(),
            spells: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct SpellSlotLevel {
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub used: u32,
}

impl SpellSlotLevel {
    pub fn available(&self) -> u32 {
        self.total.saturating_sub(self.used)
    }

    pub fn is_available(&self) -> bool {
        self.available() > 0
    }

    pub fn is_empty(&self) -> bool {
        self.available() == 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FreeUses {
    #[serde(default)]
    pub used: u32,
    #[serde(default)]
    pub max: u32,
}

impl FreeUses {
    pub fn available(&self) -> u32 {
        self.max.saturating_sub(self.used)
    }

    pub fn is_available(&self) -> bool {
        self.available() > 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Spell {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub prepared: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sticky: bool,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub free_uses: Option<FreeUses>,
}

impl Spell {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}
