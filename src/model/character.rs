use std::collections::{HashMap, HashSet};

use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::*;

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
    #[serde(default)]
    pub identity: CharacterIdentity,
    #[serde(default)]
    pub abilities: AbilityScores,
    #[serde(default)]
    pub saving_throws: HashSet<Ability>,
    #[serde(default)]
    pub skills: HashMap<Skill, ProficiencyLevel>,
    #[serde(default)]
    pub combat: CombatStats,
    #[serde(default)]
    pub personality: Personality,
    #[serde(default)]
    pub features: Vec<Feature>,
    #[serde(default)]
    pub equipment: Equipment,
    #[serde(default)]
    pub spellcasting: Option<SpellcastingData>,
    #[serde(default)]
    pub proficiencies: HashSet<Proficiency>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub racial_traits: Vec<RacialTrait>,
    #[serde(default)]
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
            .map(|c| match &c.subclass {
                Some(sc) if !sc.is_empty() => format!("{} ({sc}) {}", c.class, c.level),
                _ => format!("{} {}", c.class, c.level),
            })
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
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub subclass: Option<String>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub hit_die_sides: u16,
    #[serde(default)]
    pub hit_dice_used: u32,
    #[serde(default)]
    pub applied_levels: Vec<u32>,
}

impl Default for ClassLevel {
    fn default() -> Self {
        Self {
            class: String::new(),
            subclass: None,
            level: 1,
            hit_die_sides: 8,
            hit_dice_used: 0,
            applied_levels: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Store)]
pub struct AbilityScores {
    #[serde(default)]
    pub strength: u32,
    #[serde(default)]
    pub dexterity: u32,
    #[serde(default)]
    pub constitution: u32,
    #[serde(default)]
    pub intelligence: u32,
    #[serde(default)]
    pub wisdom: u32,
    #[serde(default)]
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Store)]
pub struct CombatStats {
    #[serde(default)]
    pub armor_class: i32,
    #[serde(default)]
    pub speed: u32,
    #[serde(default)]
    pub hp_max: i32,
    #[serde(default)]
    pub hp_current: i32,
    #[serde(default)]
    pub hp_temp: i32,
    #[serde(default)]
    pub death_save_successes: u8,
    #[serde(default)]
    pub death_save_failures: u8,
    #[serde(default)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Feature {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Equipment {
    #[serde(default)]
    pub weapons: Vec<Weapon>,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Weapon {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub attack_bonus: String,
    #[serde(default)]
    pub damage: String,
    #[serde(default)]
    pub damage_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Item {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub quantity: u32,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Currency {
    #[serde(default)]
    pub cp: u32,
    #[serde(default)]
    pub sp: u32,
    #[serde(default)]
    pub ep: u32,
    #[serde(default)]
    pub gp: u32,
    #[serde(default)]
    pub pp: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct SpellcastingData {
    pub casting_ability: Ability,
    #[serde(default)]
    pub spell_slots: Vec<SpellSlotLevel>,
    #[serde(default)]
    pub spells: Vec<Spell>,
    #[serde(default)]
    pub metamagic: Option<MetamagicData>,
}

impl SpellcastingData {
    pub fn spell_slot(&self, level: u32) -> SpellSlotLevel {
        self.spell_slots
            .get((level - 1) as usize)
            .copied()
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
    #[serde(default)]
    pub sorcery_points_max: u32,
    #[serde(default)]
    pub sorcery_points_used: u32,
    #[serde(default)]
    pub options: Vec<MetamagicOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct MetamagicOption {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct SpellSlotLevel {
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub used: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Spell {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub prepared: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct RacialTrait {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}
