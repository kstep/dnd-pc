use std::collections::{HashMap, HashSet};

use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::*;

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
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
    #[serde(default, skip_serializing_if = "is_default")]
    pub identity: CharacterIdentity,
    #[serde(default, skip_serializing_if = "is_default")]
    pub abilities: AbilityScores,
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub saving_throws: HashSet<Ability>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub skills: HashMap<Skill, ProficiencyLevel>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub combat: CombatStats,
    #[serde(default, skip_serializing_if = "is_default")]
    pub personality: Personality,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<Feature>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub equipment: Equipment,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spellcasting: Option<SpellcastingData>,
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub proficiencies: HashSet<Proficiency>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub racial_traits: Vec<RacialTrait>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
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
        let proficient = self.saving_throws.contains(&ability);
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
        Self {
            id: Uuid::new_v4(),
            identity: CharacterIdentity::default(),
            abilities: AbilityScores::default(),
            saving_throws: HashSet::new(),
            skills: HashMap::new(),
            combat: CombatStats::default(),
            personality: Personality::default(),
            features: Vec::new(),
            equipment: Equipment::default(),
            spellcasting: None,
            proficiencies: HashSet::new(),
            languages: Vec::new(),
            racial_traits: Vec::new(),
            notes: String::new(),
        }
    }
}

// --- Sub-structs ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct CharacterIdentity {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<ClassLevel>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub race: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub background: String,
    pub alignment: Alignment,
    #[serde(default, skip_serializing_if = "is_default")]
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub class: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub level: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub hit_die_sides: u16,
    #[serde(default, skip_serializing_if = "is_default")]
    pub hit_dice_used: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied_levels: Vec<u32>,
}

impl Default for ClassLevel {
    fn default() -> Self {
        Self {
            class: String::new(),
            level: 1,
            hit_die_sides: 8,
            hit_dice_used: 0,
            applied_levels: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct AbilityScores {
    #[serde(default, skip_serializing_if = "is_default")]
    pub strength: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub dexterity: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub constitution: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub intelligence: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub wisdom: u32,
    #[serde(default, skip_serializing_if = "is_default")]
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
    #[serde(default, skip_serializing_if = "is_default")]
    pub armor_class: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub speed: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub hp_max: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub hp_current: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub hp_temp: i32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub death_save_successes: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    pub death_save_failures: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    pub initiative_misc_bonus: i32,
}

impl Default for CombatStats {
    fn default() -> Self {
        Self {
            armor_class: 10,
            speed: 30,
            hp_max: 0,
            hp_current: 0,
            hp_temp: 0,
            death_save_successes: 0,
            death_save_failures: 0,
            initiative_misc_bonus: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Personality {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub history: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub personality_traits: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ideals: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bonds: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub flaws: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Feature {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Equipment {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weapons: Vec<Weapon>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<Item>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Weapon {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attack_bonus: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub damage: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub damage_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Item {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub quantity: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Currency {
    #[serde(default, skip_serializing_if = "is_default")]
    pub cp: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub sp: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub ep: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub gp: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub pp: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct SpellcastingData {
    pub casting_ability: Ability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spell_slots: Vec<SpellSlotLevel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spells: Vec<Spell>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metamagic: Option<MetamagicData>,
}

impl SpellcastingData {
    pub fn spell_slot(&self, level: u32) -> SpellSlotLevel {
        self.spell_slots
            .get((level - 1) as usize)
            .cloned()
            .unwrap_or_default()
    }

    pub fn all_spell_slots(&self) -> impl Iterator<Item = (u32, SpellSlotLevel)> + '_ {
        (1..=9u32).map(|level| (level, self.spell_slot(level)))
    }
}

impl Default for SpellcastingData {
    fn default() -> Self {
        Self {
            casting_ability: Ability::Intelligence,
            spell_slots: Vec::new(),
            spells: Vec::new(),
            metamagic: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct MetamagicData {
    #[serde(default, skip_serializing_if = "is_default")]
    pub sorcery_points_max: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub sorcery_points_used: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<MetamagicOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct MetamagicOption {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub cost: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct SpellSlotLevel {
    #[serde(default, skip_serializing_if = "is_default")]
    pub total: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub used: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Spell {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub level: u32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub prepared: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct RacialTrait {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}
