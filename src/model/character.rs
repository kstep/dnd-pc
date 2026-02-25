use std::collections::HashMap;

use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::*;

fn default_hit_die_sides() -> u16 {
    8
}

// --- Character Index (for list page) ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharacterIndex {
    pub characters: Vec<CharacterSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub id: Uuid,
    pub name: String,
    pub class: String,
    pub level: u32,
}

// --- Main Character ---

#[derive(Debug, Clone, Serialize, Deserialize, Store)]
pub struct Character {
    pub id: Uuid,
    pub identity: CharacterIdentity,
    pub abilities: AbilityScores,
    pub saving_throws: HashMap<Ability, bool>,
    pub skills: HashMap<Skill, ProficiencyLevel>,
    pub combat: CombatStats,
    pub personality: Personality,
    pub features: Vec<Feature>,
    pub equipment: Equipment,
    pub spellcasting: Option<SpellcastingData>,
    pub proficiencies: HashMap<Proficiency, bool>,
    pub languages: Vec<String>,
    pub racial_traits: Vec<RacialTrait>,
    pub notes: String,
}

impl Character {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn level(&self) -> u32 {
        self.identity
            .classes
            .iter()
            .map(|c| c.level)
            .sum::<u32>()
            .max(1)
    }

    pub fn proficiency_bonus(&self) -> i32 {
        ((self.level() as i32) - 1) / 4 + 2
    }

    pub fn ability_modifier(&self, ability: Ability) -> i32 {
        let score = self.abilities.get(ability) as i32;
        (score - 10).div_euclid(2)
    }

    pub fn saving_throw_bonus(&self, ability: Ability) -> i32 {
        let modifier = self.ability_modifier(ability);
        let proficient = self.saving_throws.get(&ability).copied().unwrap_or(false);
        modifier
            + if proficient {
                self.proficiency_bonus()
            } else {
                0
            }
    }

    pub fn skill_bonus(&self, skill: Skill) -> i32 {
        let ability = skill.ability();
        let modifier = self.ability_modifier(ability);
        let prof_level = self
            .skills
            .get(&skill)
            .copied()
            .unwrap_or(ProficiencyLevel::None);
        modifier + prof_level.multiplier() * self.proficiency_bonus()
    }

    pub fn initiative(&self) -> i32 {
        self.ability_modifier(Ability::Dexterity) + self.combat.initiative_misc_bonus
    }

    pub fn spell_save_dc(&self) -> Option<i32> {
        self.spellcasting
            .as_ref()
            .map(|sc| 8 + self.proficiency_bonus() + self.ability_modifier(sc.casting_ability))
    }

    pub fn spell_attack_bonus(&self) -> Option<i32> {
        self.spellcasting
            .as_ref()
            .map(|sc| self.proficiency_bonus() + self.ability_modifier(sc.casting_ability))
    }

    pub fn class_summary(&self) -> String {
        self.identity
            .classes
            .iter()
            .filter(|c| !c.class.is_empty())
            .map(|c| format!("{} {}", c.class, c.level))
            .collect::<Vec<_>>()
            .join(" / ")
    }

    pub fn summary(&self) -> CharacterSummary {
        CharacterSummary {
            id: self.id,
            name: self.identity.name.clone(),
            class: self.class_summary(),
            level: self.level(),
        }
    }
}

impl Default for Character {
    fn default() -> Self {
        use strum::IntoEnumIterator;

        let saving_throws = Ability::iter().map(|a| (a, false)).collect();
        let skills = Skill::iter().map(|s| (s, ProficiencyLevel::None)).collect();

        Self {
            id: Uuid::new_v4(),
            identity: CharacterIdentity::default(),
            abilities: AbilityScores::default(),
            saving_throws,
            skills,
            combat: CombatStats::default(),
            personality: Personality::default(),
            features: Vec::new(),
            equipment: Equipment::default(),
            spellcasting: None,
            proficiencies: Proficiency::iter().map(|p| (p, false)).collect(),
            languages: Vec::new(),
            racial_traits: Vec::new(),
            notes: String::new(),
        }
    }
}

// --- Sub-structs ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct CharacterIdentity {
    pub name: String,
    pub classes: Vec<ClassLevel>,
    pub race: String,
    pub background: String,
    pub alignment: Alignment,
    pub experience_points: u32,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct ClassLevel {
    pub class: String,
    pub level: u32,
    #[serde(default = "default_hit_die_sides")]
    pub hit_die_sides: u16,
    #[serde(default)]
    pub hit_dice_used: u32,
}

impl Default for ClassLevel {
    fn default() -> Self {
        Self {
            class: String::new(),
            level: 1,
            hit_die_sides: 8,
            hit_dice_used: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct AbilityScores {
    pub strength: u32,
    pub dexterity: u32,
    pub constitution: u32,
    pub intelligence: u32,
    pub wisdom: u32,
    pub charisma: u32,
}

impl AbilityScores {
    pub fn get(&self, ability: Ability) -> u32 {
        match ability {
            Ability::Strength => self.strength,
            Ability::Dexterity => self.dexterity,
            Ability::Constitution => self.constitution,
            Ability::Intelligence => self.intelligence,
            Ability::Wisdom => self.wisdom,
            Ability::Charisma => self.charisma,
        }
    }

    pub fn set(&mut self, ability: Ability, value: u32) {
        match ability {
            Ability::Strength => self.strength = value,
            Ability::Dexterity => self.dexterity = value,
            Ability::Constitution => self.constitution = value,
            Ability::Intelligence => self.intelligence = value,
            Ability::Wisdom => self.wisdom = value,
            Ability::Charisma => self.charisma = value,
        }
    }
}

impl Default for AbilityScores {
    fn default() -> Self {
        Self {
            strength: 10,
            dexterity: 10,
            constitution: 10,
            intelligence: 10,
            wisdom: 10,
            charisma: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct CombatStats {
    pub armor_class: i32,
    pub speed: u32,
    pub hp_max: i32,
    pub hp_current: i32,
    pub hp_temp: i32,
    pub death_save_successes: u8,
    pub death_save_failures: u8,
    pub initiative_misc_bonus: i32,
}

impl Default for CombatStats {
    fn default() -> Self {
        Self {
            armor_class: 10,
            speed: 30,
            hp_max: 10,
            hp_current: 10,
            hp_temp: 0,
            death_save_successes: 0,
            death_save_failures: 0,
            initiative_misc_bonus: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Personality {
    pub history: String,
    pub personality_traits: String,
    pub ideals: String,
    pub bonds: String,
    pub flaws: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Feature {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Equipment {
    pub weapons: Vec<Weapon>,
    pub items: Vec<Item>,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Weapon {
    pub name: String,
    pub attack_bonus: String,
    pub damage: String,
    pub damage_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Item {
    pub name: String,
    pub quantity: u32,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Currency {
    pub cp: u32,
    pub sp: u32,
    pub ep: u32,
    pub gp: u32,
    pub pp: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct SpellcastingData {
    pub casting_ability: Ability,
    pub spell_slots: Vec<SpellSlotLevel>,
    pub spells: Vec<Spell>,
    #[serde(default)]
    pub metamagic: Option<MetamagicData>,
}

impl Default for SpellcastingData {
    fn default() -> Self {
        Self {
            casting_ability: Ability::Intelligence,
            spell_slots: (1..=9)
                .map(|level| SpellSlotLevel {
                    level,
                    total: 0,
                    used: 0,
                })
                .collect(),
            spells: Vec::new(),
            metamagic: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct MetamagicData {
    pub sorcery_points_max: u32,
    pub sorcery_points_used: u32,
    pub options: Vec<MetamagicOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct MetamagicOption {
    pub name: String,
    pub cost: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct SpellSlotLevel {
    pub level: u32,
    pub total: u32,
    pub used: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Spell {
    pub name: String,
    pub level: u32,
    pub prepared: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct RacialTrait {
    pub name: String,
    pub description: String,
}
