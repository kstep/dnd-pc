use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dice {
    pub count: u16,
    pub sides: u16,
    pub modifier: i16,
}

impl Default for Dice {
    fn default() -> Self {
        Dice {
            count: 1,
            sides: 10,
            modifier: 0,
        }
    }
}

impl fmt::Display for Dice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}d{}", self.count, self.sides)?;
        match self.modifier.cmp(&0) {
            std::cmp::Ordering::Greater => write!(f, "+{}", self.modifier),
            std::cmp::Ordering::Less => write!(f, "{}", self.modifier),
            std::cmp::Ordering::Equal => Ok(()),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub proficiencies_and_languages: String,
    pub notes: String,
}

impl Character {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn proficiency_bonus(&self) -> i32 {
        ((self.identity.level as i32) - 1) / 4 + 2
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

    pub fn summary(&self) -> CharacterSummary {
        CharacterSummary {
            id: self.id,
            name: self.identity.name.clone(),
            class: self.identity.class.clone(),
            level: self.identity.level,
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
            proficiencies_and_languages: String::new(),
            notes: String::new(),
        }
    }
}

// --- Sub-structs ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterIdentity {
    pub name: String,
    pub class: String,
    pub level: u32,
    pub race: String,
    pub background: String,
    pub alignment: Alignment,
    pub experience_points: u32,
}

impl Default for CharacterIdentity {
    fn default() -> Self {
        Self {
            name: "New Character".to_string(),
            class: String::new(),
            level: 1,
            race: String::new(),
            background: String::new(),
            alignment: Alignment::TrueNeutral,
            experience_points: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatStats {
    pub armor_class: i32,
    pub speed: u32,
    pub hp_max: i32,
    pub hp_current: i32,
    pub hp_temp: i32,
    pub hit_dice_total: Dice,
    pub hit_dice_remaining: Dice,
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
            hit_dice_total: Dice::default(),
            hit_dice_remaining: Dice::default(),
            death_save_successes: 0,
            death_save_failures: 0,
            initiative_misc_bonus: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Personality {
    pub personality_traits: String,
    pub ideals: String,
    pub bonds: String,
    pub flaws: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Feature {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Equipment {
    pub weapons: Vec<Weapon>,
    pub items: Vec<Item>,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Weapon {
    pub name: String,
    pub attack_bonus: String,
    pub damage: String,
    pub damage_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Item {
    pub name: String,
    pub quantity: u32,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Currency {
    pub cp: u32,
    pub sp: u32,
    pub ep: u32,
    pub gp: u32,
    pub pp: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpellcastingData {
    pub casting_ability: Ability,
    pub spell_slots: Vec<SpellSlotLevel>,
    pub spells: Vec<Spell>,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpellSlotLevel {
    pub level: u32,
    pub total: u32,
    pub used: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Spell {
    pub name: String,
    pub level: u32,
    pub prepared: bool,
    pub description: String,
}
