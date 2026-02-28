use std::collections::{BTreeMap, HashMap, HashSet};

use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::*;
use crate::vecset::VecSet;

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
    pub feature_data: BTreeMap<String, FeatureData>,
    #[serde(default)]
    pub proficiencies: HashSet<Proficiency>,
    #[serde(default)]
    pub languages: VecSet<String>,
    #[serde(default)]
    pub racial_traits: Vec<RacialTrait>,
    #[serde(default)]
    pub spell_slots: Vec<SpellSlotLevel>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub updated_at: u64,
}

fn now_epoch_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

impl Character {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn touch(&mut self) {
        self.updated_at = now_epoch_secs();
    }

    pub fn caster_level(&self) -> u32 {
        self.identity
            .classes
            .iter()
            .filter(|c| c.caster_coef != 0)
            .map(|c| c.level as f32 / c.caster_coef as f32)
            .sum::<f32>()
            .floor() as u32
    }

    pub fn update_spell_slots(&mut self, slots: Option<&[u32]>) {
        let caster_classes = self
            .identity
            .classes
            .iter()
            .filter(|c| c.caster_coef != 0)
            .count();
        let slots: &[u32] = if caster_classes <= 1 {
            if let Some(s) = slots.filter(|s| !s.is_empty()) {
                s
            } else {
                let cl = self.caster_level() as usize;
                cl.checked_sub(1)
                    .and_then(|i| SPELL_SLOT_TABLE.get(i))
                    .copied()
                    .unwrap_or(&[])
            }
        } else {
            let cl = self.caster_level() as usize;
            cl.checked_sub(1)
                .and_then(|i| SPELL_SLOT_TABLE.get(i))
                .copied()
                .unwrap_or(&[])
        };
        self.spell_slots
            .resize_with(slots.len().max(self.spell_slots.len()), Default::default);
        for (i, entry) in self.spell_slots.iter_mut().enumerate() {
            entry.total = slots.get(i).copied().unwrap_or(0);
        }
        while self
            .spell_slots
            .last()
            .is_some_and(|s| s.total == 0 && s.used == 0)
        {
            self.spell_slots.pop();
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

    pub fn spell_save_dc(&self, ability: Ability) -> i32 {
        8 + self.proficiency_bonus() + self.ability_modifier(ability)
    }

    pub fn spell_attack_bonus(&self, ability: Ability) -> i32 {
        self.proficiency_bonus() + self.ability_modifier(ability)
    }

    pub fn spell_slot(&self, level: u32) -> SpellSlotLevel {
        self.spell_slots
            .get((level - 1) as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn all_spell_slots(&self) -> impl Iterator<Item = (u32, SpellSlotLevel)> + '_ {
        (1..=9u32).map(|level| (level, self.spell_slot(level)))
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
            feature_data: BTreeMap::new(),
            spell_slots: Vec::new(),
            proficiencies: HashSet::new(),
            languages: VecSet::new(),
            racial_traits: Vec::new(),
            notes: String::new(),
            updated_at: now_epoch_secs(),
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
    pub subclass: Option<String>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub hit_die_sides: u16,
    #[serde(default)]
    pub hit_dice_used: u32,
    #[serde(default)]
    pub applied_levels: VecSet<u32>,
    #[serde(default)]
    pub caster_coef: u8,
}

impl Default for ClassLevel {
    fn default() -> Self {
        Self {
            class: String::new(),
            subclass: None,
            level: 1,
            hit_die_sides: 8,
            hit_dice_used: 0,
            applied_levels: VecSet::new(),
            caster_coef: 0,
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
pub struct FeatureData {
    #[serde(default)]
    pub fields: Vec<FeatureField>,
    #[serde(default)]
    pub spells: Option<SpellData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct FeatureField {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub value: FeatureValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub enum FeatureValue {
    Points { used: u32, max: u32 },
    Choice { options: Vec<FeatureOption> },
    Die(String),
    Bonus(i32),
}

impl Default for FeatureValue {
    fn default() -> Self {
        FeatureValue::Points { used: 0, max: 0 }
    }
}

impl FeatureValue {
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
    pub description: String,
    #[serde(default)]
    pub cost: u32,
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
    #[serde(default)]
    pub sticky: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct RacialTrait {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
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
                    subclass: None,
                    level: 5,
                    hit_die_sides: 10,
                    hit_dice_used: 0,
                    applied_levels: VecSet::new(),
                    caster_coef: 0,
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
            },
            personality: Personality::default(),
            features: Vec::new(),
            equipment: Equipment::default(),
            feature_data: BTreeMap::new(),
            proficiencies: HashSet::new(),
            languages: VecSet::new(),
            racial_traits: Vec::new(),
            spell_slots: Vec::new(),
            notes: String::new(),
            updated_at: 0,
        }
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
        assert_eq!(ch.caster_level(), 0);
    }

    #[wasm_bindgen_test]
    fn caster_level_full_caster() {
        let mut ch = test_character();
        ch.identity.classes[0].caster_coef = 1;
        assert_eq!(ch.caster_level(), 5);
    }

    #[wasm_bindgen_test]
    fn caster_level_half_caster() {
        let mut ch = test_character();
        ch.identity.classes[0].caster_coef = 2;
        // 5 / 2 = 2.5 -> floor = 2
        assert_eq!(ch.caster_level(), 2);
    }

    #[wasm_bindgen_test]
    fn caster_level_multiclass() {
        let mut ch = test_character();
        ch.identity.classes[0].caster_coef = 1; // full caster, level 5
        ch.identity.classes.push(ClassLevel {
            class: "Paladin".to_string(),
            level: 4,
            caster_coef: 2, // half caster
            ..ClassLevel::default()
        });
        // 5/1 + 4/2 = 5 + 2 = 7
        assert_eq!(ch.caster_level(), 7);
    }

    // --- update_spell_slots() ---

    #[wasm_bindgen_test]
    fn update_spell_slots_single_full_caster() {
        let mut ch = test_character();
        ch.identity.classes[0].caster_coef = 1; // full caster level 5
        ch.update_spell_slots(None);
        // Caster level 5: [4, 3, 2]
        assert_eq!(ch.spell_slots.len(), 3);
        assert_eq!(ch.spell_slots[0].total, 4);
        assert_eq!(ch.spell_slots[1].total, 3);
        assert_eq!(ch.spell_slots[2].total, 2);
    }

    #[wasm_bindgen_test]
    fn update_spell_slots_with_class_override() {
        let mut ch = test_character();
        ch.identity.classes[0].caster_coef = 1;
        ch.update_spell_slots(Some(&[2, 1]));
        assert_eq!(ch.spell_slots.len(), 2);
        assert_eq!(ch.spell_slots[0].total, 2);
        assert_eq!(ch.spell_slots[1].total, 1);
    }

    #[wasm_bindgen_test]
    fn update_spell_slots_no_caster() {
        let mut ch = test_character();
        ch.update_spell_slots(None);
        assert!(ch.spell_slots.is_empty());
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
}
