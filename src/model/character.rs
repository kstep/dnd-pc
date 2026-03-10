use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    str::FromStr,
};

use indexmap::IndexMap;
use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::*;
use crate::{constvec::ConstVec, model::Money, vecset::VecSet};

// --- Die type ---

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Die {
    pub amount: u32,
    pub sides: u32,
}

impl fmt::Display for Die {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}d{}", self.amount, self.sides)
    }
}

impl FromStr for Die {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (amount, sides) = s.split_once('d').ok_or("expected {amount}d{sides}")?;
        Ok(Die {
            amount: amount.parse().map_err(|_| "invalid amount")?,
            sides: sides.parse().map_err(|_| "invalid sides")?,
        })
    }
}

impl Serialize for Die {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Die {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Spell slot table (full-caster Wizard progression), indexed by caster level
/// 1–20. Each row lists slot counts for spell levels 1–9.
pub const SPELL_SLOT_TABLE: &[&[u32]] = &[
    &[2],                         // caster level 1
    &[3],                         // 2
    &[4, 2],                      // 3
    &[4, 3],                      // 4
    &[4, 3, 2],                   // 5
    &[4, 3, 3],                   // 6
    &[4, 3, 3, 1],                // 7
    &[4, 3, 3, 2],                // 8
    &[4, 3, 3, 3, 1],             // 9
    &[4, 3, 3, 3, 2],             // 10
    &[4, 3, 3, 3, 2, 1],          // 11
    &[4, 3, 3, 3, 2, 1],          // 12
    &[4, 3, 3, 3, 2, 1, 1],       // 13
    &[4, 3, 3, 3, 2, 1, 1],       // 14
    &[4, 3, 3, 3, 2, 1, 1, 1],    // 15
    &[4, 3, 3, 3, 2, 1, 1, 1],    // 16
    &[4, 3, 3, 3, 2, 1, 1, 1, 1], // 17
    &[4, 3, 3, 3, 3, 1, 1, 1, 1], // 18
    &[4, 3, 3, 3, 3, 2, 1, 1, 1], // 19
    &[4, 3, 3, 3, 3, 2, 2, 1, 1], // 20
];

// --- Character Index (for list page) ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharacterIndex {
    #[serde(with = "index_map_as_vec")]
    pub characters: IndexMap<Uuid, CharacterSummary>,
}

/// Serialize an `IndexMap<Uuid, CharacterSummary>` as a JSON array (vec)
/// and deserialize back, preserving insertion order and O(1) lookup by id.
mod index_map_as_vec {
    use serde::ser::SerializeSeq;

    use super::*;

    pub fn serialize<S: serde::Serializer>(
        map: &IndexMap<Uuid, CharacterSummary>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(map.len()))?;
        for summary in map.values() {
            seq.serialize_element(summary)?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<IndexMap<Uuid, CharacterSummary>, D::Error> {
        let vec = Vec::<CharacterSummary>::deserialize(deserializer)?;
        Ok(vec.into_iter().map(|s| (s.id, s)).collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub id: Uuid,
    pub name: String,
    pub class: String,
    pub level: u32,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub shared: bool,
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
    pub feature_data: BTreeMap<String, FeatureData>,
    #[serde(default)]
    pub proficiencies: HashSet<Proficiency>,
    #[serde(default)]
    pub languages: VecSet<String>,
    #[serde(default)]
    pub racial_traits: Vec<RacialTrait>,
    #[serde(default)]
    pub spell_slots: BTreeMap<SpellSlotPool, ConstVec<SpellSlotLevel, 9>>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub shared: bool,
}

fn now_epoch_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

impl Character {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn long_rest(&mut self) {
        self.combat.hp_current = self.combat.hp_max;
        self.combat.hp_temp = 0;
        self.combat.death_save_successes = 0;
        self.combat.death_save_failures = 0;

        for cl in &mut self.identity.classes {
            cl.hit_dice_used = 0;
        }

        for slots in self.spell_slots.values_mut() {
            for slot in slots.iter_mut() {
                slot.used = 0;
            }
        }

        for feature_data in self.feature_data.values_mut() {
            for field in &mut feature_data.fields {
                match &mut field.value {
                    FeatureValue::Points { used, .. } | FeatureValue::Die { used, .. } => {
                        *used = 0;
                    }
                    _ => {}
                }
            }
            if let Some(spell_data) = &mut feature_data.spells {
                for spell in &mut spell_data.spells {
                    if let Some(fu) = &mut spell.free_uses {
                        fu.used = 0;
                    }
                }
            }
        }
    }

    pub fn short_rest(&mut self) {
        self.combat.death_save_failures = 0;
        self.combat.death_save_successes = 0;

        for (&pool, slots) in &mut self.spell_slots {
            if pool.restore_on_short_rest() {
                for slot in slots.iter_mut() {
                    slot.used = 0;
                }
            }
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = now_epoch_secs();
    }

    pub fn caster_level(&self, pool: SpellSlotPool) -> u32 {
        self.identity
            .classes
            .iter()
            .filter_map(|cl| {
                let max_coef = self
                    .feature_data
                    .values()
                    .filter_map(|feature_data| {
                        let spell_data = feature_data.spells.as_ref()?;
                        (spell_data.pool == pool
                            && spell_data.caster_coef != 0
                            && feature_data.source.as_ref()?.as_class() == Some(cl.class.as_str()))
                        .then_some(spell_data.caster_coef)
                    })
                    .max()?;
                // 6 is LCM(1,2,3) — the valid caster_coef values.
                // coef is the reciprocal multiplier: full=6, half=3, third=2.
                // The bitwise `& coef & 1` term rounds up for half casters
                // (divide by 2, round up) and rounds down for third casters
                // (divide by 3, round down).
                let coef = 6 / max_coef;
                Some(coef * (cl.level + (cl.level & coef & 1)))
            })
            .sum::<u32>()
            / 6
    }

    fn spell_slots_for_caster_level(&self, pool: SpellSlotPool) -> &'static [u32] {
        self.caster_level(pool)
            .checked_sub(1)
            .and_then(|level| SPELL_SLOT_TABLE.get(level as usize))
            .copied()
            .unwrap_or(&[])
    }

    pub fn highest_spell_slot_level(&self, pool: SpellSlotPool) -> u32 {
        self.spell_slots
            .get(&pool)
            .and_then(|slots| {
                slots
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, slot)| slot.total > 0)
                    .map(|(i, _)| (i + 1) as u32)
            })
            .unwrap_or(1)
    }

    pub fn update_spell_slots(&mut self, pool: SpellSlotPool, slots: Option<&[u32]>) {
        let caster_classes = self
            .identity
            .classes
            .iter()
            .filter(|cl| {
                self.feature_data.values().any(|feature_data| {
                    feature_data.spells.as_ref().is_some_and(|spell_data| {
                        spell_data.pool == pool && spell_data.caster_coef != 0
                    }) && feature_data
                        .source
                        .as_ref()
                        .and_then(|source| source.as_class())
                        == Some(cl.class.as_str())
                })
            })
            .count();

        let table_slots = self.spell_slots_for_caster_level(pool);
        let slots: &[u32] = match caster_classes {
            0 => &[],
            1 => slots.filter(|s| !s.is_empty()).unwrap_or(table_slots),
            _ => table_slots,
        };

        if !slots.is_empty() {
            let slot_entry = self.spell_slots.entry(pool).or_default();
            for (i, entry) in slot_entry.iter_mut().enumerate() {
                let table_total = slots.get(i).copied().unwrap_or(0);
                entry.total = table_total;
            }
        }
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
        self.abilities.modifier(ability)
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

    pub fn spell_save_dc(&self, ability: Ability) -> i32 {
        8 + self.proficiency_bonus() + self.ability_modifier(ability)
    }

    pub fn spell_attack_bonus(&self, ability: Ability) -> i32 {
        self.proficiency_bonus() + self.ability_modifier(ability)
    }

    pub fn spell_slot(&self, pool: SpellSlotPool, level: u32) -> SpellSlotLevel {
        self.spell_slots
            .get(&pool)
            .and_then(|slots| slots.get((level - 1) as usize))
            .copied()
            .unwrap_or_default()
    }

    pub fn all_spell_slots_for_pool(
        &self,
        pool: SpellSlotPool,
    ) -> impl Iterator<Item = (u32, SpellSlotLevel)> + '_ {
        (1..=9u32).map(move |level| (level, self.spell_slot(pool, level)))
    }

    pub fn active_pools(&self) -> impl Iterator<Item = SpellSlotPool> + '_ {
        self.spell_slots
            .iter()
            .filter(|(_, slots)| slots.iter().any(|slot| slot.total > 0))
            .map(|(&pool, _)| pool)
    }

    /// Clear all labels and descriptions (blanket clear).
    pub fn clear_all_labels(&mut self) {
        for cl in &mut self.identity.classes {
            cl.class_label = None;
            cl.subclass_label = None;
        }
        for feature in &mut self.features {
            feature.label = None;
            feature.description.clear();
        }
        for racial_trait in &mut self.racial_traits {
            racial_trait.label = None;
            racial_trait.description.clear();
        }
        for entry in self.feature_data.values_mut() {
            for field in &mut entry.fields {
                field.label = None;
                field.description.clear();
                for opt in field.value.choices_mut() {
                    opt.label = None;
                    opt.description.clear();
                }
            }
            if let Some(spells) = &mut entry.spells {
                for spell in &mut spells.spells {
                    spell.label = None;
                    spell.description.clear();
                }
            }
        }
    }

    pub fn class_summary(&self) -> String {
        self.identity
            .classes
            .iter()
            .filter(|c| !c.class.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" / ")
    }

    pub fn summary(&self) -> CharacterSummary {
        CharacterSummary {
            id: self.id,
            name: self.identity.name.clone(),
            class: self.class_summary(),
            level: self.level(),
            updated_at: self.updated_at,
            shared: self.shared,
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
            feature_data: BTreeMap::new(),
            spell_slots: BTreeMap::new(),
            proficiencies: HashSet::new(),
            languages: VecSet::new(),
            racial_traits: Vec::new(),
            notes: String::new(),
            updated_at: now_epoch_secs(),
            shared: false,
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

    pub fn modifier(&self, ability: Ability) -> i32 {
        let score = self.get(ability) as i32;
        (score - 10).div_euclid(2)
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
    pub hp_max: u32,
    #[serde(default)]
    pub hp_current: u32,
    #[serde(default)]
    pub hp_temp: u32,
    #[serde(default)]
    pub death_save_successes: u8,
    #[serde(default)]
    pub death_save_failures: u8,
    #[serde(default)]
    pub initiative_misc_bonus: i32,
    #[serde(default)]
    pub inspiration: bool,
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
            inspiration: false,
        }
    }
}

impl CombatStats {
    pub fn damage(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }

        let amount = if self.hp_temp > 0 {
            let temp_absorb = self.hp_temp.min(amount);
            self.hp_temp -= temp_absorb;
            amount - temp_absorb
        } else {
            amount
        };

        self.hp_current = self.hp_current.saturating_sub(amount);
    }

    pub fn heal(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }

        self.hp_current = (self.hp_current + amount).min(self.hp_max);
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
pub struct Equipment {
    #[serde(default)]
    pub weapons: Vec<Weapon>,
    #[serde(default)]
    pub armors: Vec<Armor>,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Armor {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub base_ac: u32,
    #[serde(default)]
    pub armor_type: ArmorType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Weapon {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub attack_bonus: i32,
    #[serde(default)]
    pub damage: String,
    #[serde(default)]
    pub damage_type: Option<DamageType>,
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

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if self.quantity > 1 {
            write!(f, " \u{00d7}{}", self.quantity)?;
        }
        Ok(())
    }
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

impl Currency {
    pub fn as_money(&self) -> Money {
        Money::from_cp(
            self.cp
                + self.sp * Money::CP_PER_SP
                + self.ep * Money::CP_PER_EP
                + self.gp * Money::CP_PER_GP
                + self.pp * Money::CP_PER_PP,
        )
    }

    pub fn gain(&mut self, amount: Money) {
        let (gain_gp, gain_sp, gain_cp) = amount.as_gp_sp_cp();
        self.cp += gain_cp;
        self.sp += gain_sp;
        self.gp += gain_gp;
    }

    #[allow(unused_assignments)]
    pub fn spend(&mut self, amount: Money) -> bool {
        if amount > self.as_money() {
            return false;
        }

        let mut remaining_cp = amount.whole_cp();

        macro_rules! spend_coin {
            ($coin:ident, $cp_per:expr) => {
                if remaining_cp > 0 {
                    let can_spend = (remaining_cp / $cp_per).min(self.$coin);
                    self.$coin -= can_spend;
                    remaining_cp -= can_spend * $cp_per;
                }
            };
        }

        spend_coin!(pp, Money::CP_PER_PP);
        spend_coin!(gp, Money::CP_PER_GP);
        spend_coin!(ep, Money::CP_PER_EP);
        spend_coin!(sp, Money::CP_PER_SP);
        spend_coin!(cp, 1u32);

        // If there's still a remainder, break the smallest available coin that
        // covers it and give change back in GP/SP/CP (no EP to keep it clean).
        // The three guards: still something to spend, coin is in wallet, coin
        // covers the remainder (one coin is enough since the greedy pass already
        // consumed all coins whose denomination divides evenly into remaining_cp).
        if remaining_cp > 0 {
            macro_rules! break_coin {
                ($coin:ident, $cp_per:expr) => {
                    if remaining_cp > 0 && self.$coin > 0 && $cp_per >= remaining_cp {
                        self.$coin -= 1;
                        let mut change = $cp_per - remaining_cp;
                        self.gp += change / Money::CP_PER_GP;
                        change %= Money::CP_PER_GP;
                        self.sp += change / Money::CP_PER_SP;
                        self.cp += change % Money::CP_PER_SP;
                        remaining_cp = 0;
                    }
                };
            }

            break_coin!(sp, Money::CP_PER_SP);
            break_coin!(ep, Money::CP_PER_EP);
            break_coin!(gp, Money::CP_PER_GP);
            break_coin!(pp, Money::CP_PER_PP);
        }

        true
    }
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for (amount, label) in [
            (self.pp, "pp"),
            (self.gp, "gp"),
            (self.ep, "ep"),
            (self.sp, "sp"),
            (self.cp, "cp"),
        ] {
            if amount > 0 {
                if !first {
                    f.write_str(" ")?;
                }
                write!(f, "{amount}{label}")?;
                first = false;
            }
        }
        if first {
            f.write_str("\u{2014}")?;
        }
        Ok(())
    }
}

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

#[cfg(test)]
pub mod tests {
    use std::collections::{BTreeMap, HashMap, HashSet};

    use wasm_bindgen_test::*;

    use super::*;
    use crate::vecset::VecSet;

    /// Build a minimal character for testing (avoids Default which calls
    /// js_sys::Date)
    fn test_character() -> Character {
        Character {
            id: Uuid::nil(),
            identity: CharacterIdentity {
                name: "Test".to_string(),
                classes: vec![ClassLevel {
                    class: "Fighter".to_string(),
                    class_label: None,
                    subclass: None,
                    subclass_label: None,
                    level: 5,
                    hit_die_sides: 10,
                    hit_dice_used: 0,
                    applied_levels: VecSet::new(),
                }],
                race: "Human".to_string(),
                background: "Soldier".to_string(),
                alignment: Alignment::TrueNeutral,
                experience_points: 0,
                race_applied: false,
                background_applied: false,
            },
            abilities: AbilityScores {
                strength: 16,
                dexterity: 14,
                constitution: 12,
                intelligence: 10,
                wisdom: 8,
                charisma: 13,
            },
            saving_throws: HashSet::from([Ability::Strength, Ability::Constitution]),
            skills: HashMap::from([
                (Skill::Athletics, ProficiencyLevel::Proficient),
                (Skill::Perception, ProficiencyLevel::Expertise),
            ]),
            combat: CombatStats {
                armor_class: 18,
                speed: 30,
                hp_max: 44,
                hp_current: 44,
                hp_temp: 0,
                death_save_successes: 0,
                death_save_failures: 0,
                initiative_misc_bonus: 0,
                inspiration: false,
            },
            personality: Personality::default(),
            features: Vec::new(),
            equipment: Equipment::default(),
            feature_data: BTreeMap::new(),
            proficiencies: HashSet::new(),
            languages: VecSet::new(),
            racial_traits: Vec::new(),
            spell_slots: BTreeMap::new(),
            notes: String::new(),
            updated_at: 0,
            shared: false,
        }
    }

    /// Helper: set up a character as a caster by adding SpellData with source
    fn make_caster(
        ch: &mut Character,
        class_name: &str,
        feature_name: &str,
        caster_coef: u32,
        pool: SpellSlotPool,
    ) {
        ch.feature_data.insert(
            feature_name.to_string(),
            FeatureData {
                source: Some(FeatureSource::Class(class_name.to_string())),
                spells: Some(SpellData {
                    casting_ability: Ability::Intelligence,
                    caster_coef,
                    pool,
                    spells: Vec::new(),
                }),
                ..Default::default()
            },
        );
    }

    // --- level() ---

    #[wasm_bindgen_test]
    fn level_single_class() {
        let ch = test_character();
        assert_eq!(ch.level(), 5);
    }

    #[wasm_bindgen_test]
    fn level_multiclass() {
        let mut ch = test_character();
        ch.identity.classes.push(ClassLevel {
            class: "Wizard".to_string(),
            level: 3,
            ..ClassLevel::default()
        });
        assert_eq!(ch.level(), 8);
    }

    #[wasm_bindgen_test]
    fn level_no_classes_returns_1() {
        let mut ch = test_character();
        ch.identity.classes.clear();
        assert_eq!(ch.level(), 1);
    }

    // --- proficiency_bonus() ---

    #[wasm_bindgen_test]
    fn proficiency_bonus_levels() {
        let mut ch = test_character();
        let expected = [
            (1, 2),
            (4, 2),
            (5, 3),
            (8, 3),
            (9, 4),
            (12, 4),
            (13, 5),
            (16, 5),
            (17, 6),
            (20, 6),
        ];
        for (level, bonus) in expected {
            ch.identity.classes[0].level = level;
            assert_eq!(ch.proficiency_bonus(), bonus, "level {level}");
        }
    }

    // --- ability_modifier() ---

    #[wasm_bindgen_test]
    fn ability_modifier_values() {
        let ch = test_character();
        // STR 16 -> +3, DEX 14 -> +2, CON 12 -> +1, INT 10 -> 0, WIS 8 -> -1, CHA 13 ->
        // +1
        assert_eq!(ch.ability_modifier(Ability::Strength), 3);
        assert_eq!(ch.ability_modifier(Ability::Dexterity), 2);
        assert_eq!(ch.ability_modifier(Ability::Constitution), 1);
        assert_eq!(ch.ability_modifier(Ability::Intelligence), 0);
        assert_eq!(ch.ability_modifier(Ability::Wisdom), -1);
        assert_eq!(ch.ability_modifier(Ability::Charisma), 1);
    }

    #[wasm_bindgen_test]
    fn ability_modifier_odd_scores() {
        let mut ch = test_character();
        // score 1 -> -5, score 9 -> -1, score 11 -> 0, score 20 -> +5
        let cases = [(1, -5), (9, -1), (11, 0), (20, 5)];
        for (score, expected_mod) in cases {
            ch.abilities.strength = score;
            assert_eq!(
                ch.ability_modifier(Ability::Strength),
                expected_mod,
                "score {score}"
            );
        }
    }

    // --- skill_bonus() ---

    #[wasm_bindgen_test]
    fn skill_bonus_no_proficiency() {
        let ch = test_character();
        // Stealth: DEX mod (+2), no proficiency
        assert_eq!(ch.skill_bonus(Skill::Stealth), 2);
    }

    #[wasm_bindgen_test]
    fn skill_bonus_proficient() {
        let ch = test_character();
        // Athletics: STR mod (+3) + proficiency bonus (3) = 6
        assert_eq!(ch.skill_bonus(Skill::Athletics), 6);
    }

    #[wasm_bindgen_test]
    fn skill_bonus_expertise() {
        let ch = test_character();
        // Perception: WIS mod (-1) + 2 * proficiency bonus (3) = -1 + 6 = 5
        assert_eq!(ch.skill_bonus(Skill::Perception), 5);
    }

    // --- saving_throw_bonus() ---

    #[wasm_bindgen_test]
    fn saving_throw_proficient() {
        let ch = test_character();
        // STR: mod (+3) + prof bonus (3) = 6
        assert_eq!(ch.saving_throw_bonus(Ability::Strength), 6);
    }

    #[wasm_bindgen_test]
    fn saving_throw_not_proficient() {
        let ch = test_character();
        // DEX: mod (+2) only
        assert_eq!(ch.saving_throw_bonus(Ability::Dexterity), 2);
    }

    // --- initiative() ---

    #[wasm_bindgen_test]
    fn initiative_basic() {
        let ch = test_character();
        // DEX mod (+2) + misc (0)
        assert_eq!(ch.initiative(), 2);
    }

    #[wasm_bindgen_test]
    fn initiative_with_misc_bonus() {
        let mut ch = test_character();
        ch.combat.initiative_misc_bonus = 3;
        assert_eq!(ch.initiative(), 5);
    }

    // --- spell_save_dc() and spell_attack_bonus() ---

    #[wasm_bindgen_test]
    fn spell_save_dc() {
        let ch = test_character();
        // 8 + prof (3) + WIS mod (-1) = 10
        assert_eq!(ch.spell_save_dc(Ability::Wisdom), 10);
    }

    #[wasm_bindgen_test]
    fn spell_attack_bonus() {
        let ch = test_character();
        // prof (3) + CHA mod (+1) = 4
        assert_eq!(ch.spell_attack_bonus(Ability::Charisma), 4);
    }

    // --- caster_level() ---

    #[wasm_bindgen_test]
    fn caster_level_no_caster() {
        let ch = test_character();
        assert_eq!(ch.caster_level(SpellSlotPool::Arcane), 0);
    }

    #[wasm_bindgen_test]
    fn caster_level_full_caster() {
        let mut ch = test_character();
        make_caster(&mut ch, "Fighter", "Spellcasting", 1, SpellSlotPool::Arcane);
        assert_eq!(ch.caster_level(SpellSlotPool::Arcane), 5);
    }

    #[wasm_bindgen_test]
    fn caster_level_half_caster() {
        let mut ch = test_character();
        make_caster(&mut ch, "Fighter", "Spellcasting", 2, SpellSlotPool::Arcane);
        // 5 / 2 = 3 (rounds up for odd levels)
        assert_eq!(ch.caster_level(SpellSlotPool::Arcane), 3);
    }

    #[wasm_bindgen_test]
    fn caster_level_multiclass() {
        let mut ch = test_character();
        make_caster(
            &mut ch,
            "Fighter",
            "Spellcasting (Fighter)",
            1,
            SpellSlotPool::Arcane,
        );
        ch.identity.classes.push(ClassLevel {
            class: "Paladin".to_string(),
            level: 4,
            ..ClassLevel::default()
        });
        make_caster(
            &mut ch,
            "Paladin",
            "Spellcasting (Paladin)",
            2,
            SpellSlotPool::Arcane,
        );
        // 5/1 + 4/2 = 5 + 2 = 7
        assert_eq!(ch.caster_level(SpellSlotPool::Arcane), 7);
    }

    #[wasm_bindgen_test]
    fn caster_level_pact_pool_separate() {
        let mut ch = test_character();
        make_caster(
            &mut ch,
            "Fighter",
            "Spellcasting (Fighter)",
            1,
            SpellSlotPool::Arcane,
        );
        ch.identity.classes.push(ClassLevel {
            class: "Warlock".to_string(),
            level: 3,
            ..ClassLevel::default()
        });
        make_caster(&mut ch, "Warlock", "Pact Magic", 1, SpellSlotPool::Pact);
        // Arcane pool only sees Fighter
        assert_eq!(ch.caster_level(SpellSlotPool::Arcane), 5);
        // Pact pool only sees Warlock
        assert_eq!(ch.caster_level(SpellSlotPool::Pact), 3);
    }

    // --- update_spell_slots() ---

    #[wasm_bindgen_test]
    fn update_spell_slots_single_full_caster() {
        let mut ch = test_character();
        make_caster(&mut ch, "Fighter", "Spellcasting", 1, SpellSlotPool::Arcane);
        ch.update_spell_slots(SpellSlotPool::Arcane, None);
        let slots = &ch.spell_slots[&SpellSlotPool::Arcane];
        // Caster level 5: [4, 3, 2]; trailing zeros trimmed
        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0].total, 4);
        assert_eq!(slots[1].total, 3);
        assert_eq!(slots[2].total, 2);
    }

    #[wasm_bindgen_test]
    fn update_spell_slots_with_class_override() {
        let mut ch = test_character();
        make_caster(&mut ch, "Fighter", "Spellcasting", 1, SpellSlotPool::Arcane);
        ch.update_spell_slots(SpellSlotPool::Arcane, Some(&[2, 1]));
        let slots = &ch.spell_slots[&SpellSlotPool::Arcane];
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].total, 2);
        assert_eq!(slots[1].total, 1);
    }

    #[wasm_bindgen_test]
    fn update_spell_slots_no_caster() {
        let mut ch = test_character();
        ch.update_spell_slots(SpellSlotPool::Arcane, None);
        assert!(ch.spell_slots.is_empty() || ch.spell_slots[&SpellSlotPool::Arcane].is_empty());
    }

    #[wasm_bindgen_test]
    fn update_spell_slots_recalculates_totals() {
        let mut ch = test_character();
        make_caster(&mut ch, "Fighter", "Spellcasting", 1, SpellSlotPool::Arcane);
        ch.update_spell_slots(SpellSlotPool::Arcane, None);
        ch.spell_slots.get_mut(&SpellSlotPool::Arcane).unwrap()[0].total = 10;
        ch.update_spell_slots(SpellSlotPool::Arcane, None);
        let slots = &ch.spell_slots[&SpellSlotPool::Arcane];
        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0].total, 4); // recalculated from table
        assert_eq!(slots[1].total, 3); // from table
        assert_eq!(slots[2].total, 2); // from table
    }

    #[wasm_bindgen_test]
    fn update_spell_slots_pact_slots_replaced_on_level_up() {
        let mut ch = test_character();
        ch.identity.classes[0] = ClassLevel {
            class: "Warlock".to_string(),
            level: 9,
            ..ClassLevel::default()
        };
        make_caster(&mut ch, "Warlock", "Pact Magic", 3, SpellSlotPool::Pact);

        // Level 7: 2 slots at 4th level
        ch.update_spell_slots(SpellSlotPool::Pact, Some(&[0, 0, 0, 2]));
        let slots = &ch.spell_slots[&SpellSlotPool::Pact];
        assert_eq!(slots[3].total, 2);

        // Level 9: 2 slots at 5th level, none at 4th
        ch.update_spell_slots(SpellSlotPool::Pact, Some(&[0, 0, 0, 0, 2]));
        let slots = &ch.spell_slots[&SpellSlotPool::Pact];
        assert_eq!(slots[3].total, 0); // old 4th-level slots cleared
        assert_eq!(slots[4].total, 2); // new 5th-level slots
    }

    // --- class_summary() ---

    #[wasm_bindgen_test]
    fn class_summary_single() {
        let ch = test_character();
        assert_eq!(ch.class_summary(), "Fighter 5");
    }

    #[wasm_bindgen_test]
    fn class_summary_with_subclass() {
        let mut ch = test_character();
        ch.identity.classes[0].subclass = Some("Champion".to_string());
        assert_eq!(ch.class_summary(), "Fighter (Champion) 5");
    }

    #[wasm_bindgen_test]
    fn class_summary_multiclass() {
        let mut ch = test_character();
        ch.identity.classes.push(ClassLevel {
            class: "Rogue".to_string(),
            level: 3,
            ..ClassLevel::default()
        });
        assert_eq!(ch.class_summary(), "Fighter 5 / Rogue 3");
    }

    #[wasm_bindgen_test]
    fn class_summary_skips_empty_class() {
        let mut ch = test_character();
        ch.identity.classes.push(ClassLevel::default());
        // Default ClassLevel has empty class name, should be skipped
        assert_eq!(ch.class_summary(), "Fighter 5");
    }

    // --- Currency::spend() ---

    #[wasm_bindgen_test]
    fn currency_spend_exact_denomination() {
        let mut c = Currency {
            gp: 10,
            sp: 5,
            ..Default::default()
        };
        assert!(c.spend(Money::from_sp(5)));
        assert_eq!(
            c,
            Currency {
                gp: 10,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_breaks_higher_coin() {
        // 10 gp 0 sp — spend 5 sp should exchange 1 gp → 10 sp, leaving 9 gp 5 sp
        let mut c = Currency {
            gp: 10,
            ..Default::default()
        };
        assert!(c.spend(Money::from_sp(5)));
        assert_eq!(
            c,
            Currency {
                gp: 9,
                sp: 5,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_insufficient_returns_false() {
        let mut c = Currency {
            gp: 1,
            ..Default::default()
        };
        assert!(!c.spend(Money::from_gp(2)));
        // Currency unchanged
        assert_eq!(
            c,
            Currency {
                gp: 1,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_exact_total() {
        let mut c = Currency {
            gp: 1,
            sp: 5,
            cp: 3,
            ..Default::default()
        };
        let total = c.as_money();
        assert!(c.spend(total));
        assert_eq!(c, Currency::default());
    }

    #[wasm_bindgen_test]
    fn currency_spend_cp_from_sp() {
        // 0 cp, 1 sp → spend 5 cp → break 1 sp, return 5 cp change
        let mut c = Currency {
            sp: 1,
            ..Default::default()
        };
        assert!(c.spend(Money::from_cp(5)));
        assert_eq!(
            c,
            Currency {
                cp: 5,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_cp_exact() {
        // Spend CP when CP is available
        let mut c = Currency {
            cp: 10,
            ..Default::default()
        };
        assert!(c.spend(Money::from_cp(7)));
        assert_eq!(
            c,
            Currency {
                cp: 3,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_sp_from_ep() {
        // 1 ep 0 sp → spend 3 sp (30 cp) → break 1 ep, return 2 sp change
        let mut c = Currency {
            ep: 1,
            ..Default::default()
        };
        assert!(c.spend(Money::from_sp(3)));
        assert_eq!(
            c,
            Currency {
                sp: 2,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_ep_exact() {
        // 2 ep → spend 1 ep → 1 ep (exact match, no break needed)
        let mut c = Currency {
            ep: 2,
            sp: 3,
            ..Default::default()
        };
        assert!(c.spend(Money::from_ep(1)));
        assert_eq!(
            c,
            Currency {
                ep: 1,
                sp: 3,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_cp_from_gp() {
        // 1 gp → spend 7 cp → break 1 gp, return 9 sp 3 cp change (no EP)
        let mut c = Currency {
            gp: 1,
            ..Default::default()
        };
        assert!(c.spend(Money::from_cp(7)));
        assert_eq!(
            c,
            Currency {
                sp: 9,
                cp: 3,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_sp_from_pp_no_ep_in_change() {
        // 1 pp → spend 3 sp (30 cp) → break 1 pp, return 9 gp 7 sp (no EP)
        let mut c = Currency {
            pp: 1,
            ..Default::default()
        };
        assert!(c.spend(Money::from_sp(3)));
        assert_eq!(
            c,
            Currency {
                gp: 9,
                sp: 7,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_partial_then_break() {
        // 2 gp 3 sp → spend 15 sp (150 cp) → spend 1 gp + 3 sp, break 1 gp for 8 sp
        // change
        let mut c = Currency {
            gp: 2,
            sp: 3,
            ..Default::default()
        };
        assert!(c.spend(Money::from_sp(15)));
        assert_eq!(
            c,
            Currency {
                sp: 8,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_pp_exact() {
        // 2 pp → spend 1 pp → 1 pp
        let mut c = Currency {
            pp: 2,
            ..Default::default()
        };
        assert!(c.spend(Money::from_pp(1)));
        assert_eq!(
            c,
            Currency {
                pp: 1,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_zero() {
        // Spending 0 always succeeds and leaves currency unchanged
        let mut c = Currency {
            gp: 5,
            sp: 3,
            ..Default::default()
        };
        assert!(c.spend(Money::default()));
        assert_eq!(
            c,
            Currency {
                gp: 5,
                sp: 3,
                ..Default::default()
            }
        );
    }
}
