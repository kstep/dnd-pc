use std::collections::BTreeMap;

use indexmap::IndexMap;
use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    constvec::ConstVec,
    demap::{self, Keyed},
    expr::{self, Eval as _},
    model::{
        AbilityScores, Attribute, CharacterIdentity, CombatStats, DamageModifiers, Equipment,
        Feature, FeatureData, FeatureSource, FeatureValue, Features, Personality, SpellSlotLevel,
        enums::*,
    },
    vecset::VecSet,
};

/// Default walking speed in feet (most species).
const DEFAULT_SPEED: u32 = 30;

/// Proficiency bonus for a given character level (D&D 5e standard
/// progression).
pub fn proficiency_bonus_for_level(level: u32) -> i32 {
    (level as i32 - 1) / 4 + 2
}

/// XP thresholds for character levels 1–20 (D&D 5e standard progression).
const XP_THRESHOLDS: [u32; 20] = [
    0, 300, 900, 2_700, 6_500, 14_000, 23_000, 34_000, 48_000, 64_000, 85_000, 100_000, 120_000,
    140_000, 165_000, 195_000, 225_000, 265_000, 305_000, 355_000,
];

/// Spell slot table (full-caster Wizard progression), indexed by caster level
/// 1–20. Each row lists slot counts for spell levels 1–9.
const SPELL_SLOT_TABLE: &[&[u32]] = &[
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
    #[serde(with = "demap::index_map_as_vec")]
    pub characters: IndexMap<Uuid, CharacterSummary>,
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

impl Keyed for CharacterSummary {
    fn key(&self) -> Uuid {
        self.id
    }
}

// --- Main Character ---

#[derive(Debug, Clone, Serialize, Deserialize, Store)]
pub struct Character {
    pub id: Uuid,
    #[serde(default)]
    pub identity: CharacterIdentity,
    #[serde(default)]
    abilities: AbilityScores,
    #[serde(default)]
    saving_throws: VecSet<Ability>,
    #[serde(default)]
    skills: BTreeMap<Skill, ProficiencyLevel>,
    #[serde(default)]
    pub combat: CombatStats,
    #[serde(default)]
    pub personality: Personality,
    #[serde(default)]
    pub features: Features,
    #[serde(default)]
    pub equipment: Equipment,
    #[serde(default)]
    pub feature_data: BTreeMap<String, FeatureData>,
    #[serde(default)]
    pub proficiencies: VecSet<Proficiency>,
    #[serde(default)]
    pub languages: VecSet<String>,
    #[serde(default)]
    pub damage_modifiers: BTreeMap<DamageType, DamageModifiers>,
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

    pub fn clear(&mut self) {
        let id = self.id;
        let mut identity = std::mem::take(&mut self.identity);
        identity.species_applied = false;
        identity.background_applied = false;
        for class_level in &mut identity.classes {
            class_level.applied_levels.clear();
        }
        *self = Self {
            id,
            identity,
            ..Default::default()
        };
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

    pub fn ability_score(&self, ability: Ability) -> u32 {
        self.abilities.get(ability)
    }

    pub fn modify_ability(&mut self, ability: Ability, delta: i32) {
        let current = self.abilities.get(ability) as i32;
        self.abilities.set(ability, (current + delta).max(1) as u32);
    }

    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    pub fn speed(&self) -> u32 {
        self.combat.speed
    }

    pub fn hp_max(&self) -> u32 {
        self.combat.hp_max
    }

    pub fn gain_hp_max(&mut self, amount: i32) {
        self.combat.hp_max = self.combat.hp_max.saturating_add_signed(amount);
    }

    pub fn hp_current(&self) -> u32 {
        self.combat.hp_current
    }

    pub fn hp_temp(&self) -> u32 {
        self.combat.hp_temp
    }

    pub fn armor_class(&self) -> u32 {
        self.combat.armor_class
    }

    /// Evaluate all armor AC formulas and write the best result to
    /// `combat.armor_class`. Returns the computed AC.
    ///
    /// Evaluation order:
    /// 1. All non-shield armor formulas → pick the max (or `10 + DEX.MOD`)
    /// 2. Set AC so shield formulas can read it
    /// 3. All shield formulas → pick the max
    pub fn compute_armor_class(&mut self) -> u32 {
        let default_ac = (10 + self.ability_modifier(Ability::Dexterity)).max(0) as u32;

        // Best body armor (non-shield), skipping armor the character isn't proficient
        // with
        let body_ac = self
            .equipment
            .armors
            .iter()
            .filter(|a| a.armor_type != ArmorType::Shield)
            .filter(|a| {
                a.armor_type
                    .required_proficiency()
                    .is_none_or(|p| self.proficiencies.contains(&p))
            })
            .filter_map(|a| {
                let expr = a.ac_expr.as_ref()?;
                match expr.eval(self) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        log::warn!("AC expr eval failed for '{}': {error}", a.name);
                        None
                    }
                }
            })
            .max()
            .map(|v| v.max(0) as u32)
            .unwrap_or(default_ac);

        // Set AC so shield formulas can read it via resolve(Ac)
        self.combat.armor_class = body_ac;

        // Best shield (reads AC = body_ac), only if proficient with shields
        if !self.proficiencies.contains(&Proficiency::Shields) {
            return self.combat.armor_class;
        }
        if let Some(shield_ac) = self
            .equipment
            .armors
            .iter()
            .filter(|a| a.armor_type == ArmorType::Shield)
            .filter_map(|a| {
                let expr = a.ac_expr.as_ref()?;
                match expr.eval(self) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        log::warn!("AC expr eval failed for '{}': {error}", a.name);
                        None
                    }
                }
            })
            .max()
        {
            self.combat.armor_class = shield_ac.max(0) as u32;
        }

        self.combat.armor_class
    }

    /// Compute base max HP from class levels and CON modifier.
    ///
    /// Formula: for each class, `hit_die_sides` at level 1 +
    /// `avg_hp(hit_die_sides)` for each subsequent level, plus
    /// `total_level * CON modifier`.
    pub fn compute_hp_max(&mut self) -> u32 {
        let con_mod = self.ability_modifier(Ability::Constitution);
        let mut total_level: i32 = 0;
        let base: i32 = self
            .identity
            .classes
            .iter()
            .map(|cl| {
                total_level += cl.level as i32;
                let sides = cl.hit_die_sides as i32;
                sides + (cl.level as i32 - 1) * expr::avg_hp(sides)
            })
            .sum();
        let total = (base + total_level * con_mod).max(0) as u32;
        self.combat.hp_max = total;
        total
    }

    /// Reset speed to the default walking speed (30 ft).
    /// Race/feature `OnCompute` assignments override this.
    pub fn compute_speed(&mut self) -> u32 {
        self.combat.speed = DEFAULT_SPEED;
        DEFAULT_SPEED
    }

    /// Recompute all base combat stats (AC, HP, speed).
    /// Call `RulesRegistry::assign(character, OnCompute)` after this
    /// to apply feature bonuses.
    pub fn compute(&mut self) {
        self.compute_armor_class();
        self.compute_hp_max();
        self.compute_speed();
        self.combat.initiative_misc_bonus = 0;
        self.combat.attack_count = 1;
    }

    /// Returns (caster_level, caster_class_count) for the given pool in a
    /// single pass.
    fn caster_info(&self, pool: SpellSlotPool) -> (u32, u32) {
        let mut caster_level_sixths = 0u32;
        let mut caster_classes = 0u32;
        for cl in &self.identity.classes {
            let max_coef = self
                .features
                .iter()
                .filter_map(|feature| {
                    if feature.source.as_class() != Some(cl.class.as_str()) {
                        return None;
                    }
                    let spell_data = self.feature_data.get(&feature.name)?.spells.as_ref()?;
                    (spell_data.pool == pool && spell_data.caster_coef != 0)
                        .then_some(spell_data.caster_coef)
                })
                .max();
            if let Some(max_coef) = max_coef {
                caster_classes += 1;
                // 6 is LCM(1,2,3) — the valid caster_coef values.
                // coef is the reciprocal multiplier: full=6, half=3, third=2.
                // The bitwise `& coef & 1` term rounds up for half casters
                // (divide by 2, round up) and rounds down for third casters
                // (divide by 3, round down).
                let coef = 6 / max_coef;
                caster_level_sixths += coef * (cl.level + (cl.level & coef & 1));
            }
        }
        (caster_level_sixths / 6, caster_classes)
    }

    pub fn caster_level(&self, pool: SpellSlotPool) -> u32 {
        self.caster_info(pool).0
    }

    fn spell_slots_for_caster_level(&self, pool: SpellSlotPool) -> &'static [u32] {
        self.caster_level(pool)
            .checked_sub(1)
            .and_then(|level| SPELL_SLOT_TABLE.get(level as usize))
            .copied()
            .unwrap_or(&[])
    }

    pub fn update_spell_slots(&mut self, pool: SpellSlotPool, slots: Option<&[u32]>) {
        let (_, caster_classes) = self.caster_info(pool);

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

    pub fn can_level_up(&self) -> bool {
        !self.identity.classes.is_empty()
            && self.identity.classes.iter().all(|cl| !cl.class.is_empty())
    }

    pub fn level(&self) -> u32 {
        self.identity
            .classes
            .iter()
            .map(|c| c.level)
            .sum::<u32>()
            .max(1)
    }

    /// Effective current level for a feature based on its source.
    /// Class features use their class's current level; others use total level.
    pub fn effective_level_for(&self, source: &FeatureSource) -> u32 {
        match source {
            FeatureSource::Class(class_name, _) => self
                .identity
                .classes
                .iter()
                .find(|cl| cl.class == *class_name)
                .map_or(0, |cl| cl.level),
            FeatureSource::Species(_) | FeatureSource::Background(_) | FeatureSource::User(_) => {
                self.level()
            }
        }
    }

    pub fn xp_threshold(&self) -> u32 {
        XP_THRESHOLDS
            .get(self.level().saturating_sub(1) as usize)
            .copied()
            .unwrap_or(0)
    }

    pub fn proficiency_bonus(&self) -> i32 {
        proficiency_bonus_for_level(self.level())
    }

    pub fn ability_modifier(&self, ability: Ability) -> i32 {
        self.abilities.modifier(ability)
    }

    pub fn proficient_with(&self, ability: Ability) -> bool {
        self.saving_throws.contains(&ability)
    }

    pub fn update_saving_throw_proficiencies(&mut self, f: impl FnOnce(&mut VecSet<Ability>)) {
        f(&mut self.saving_throws);
    }

    pub fn update_skill_proficiencies(
        &mut self,
        f: impl FnOnce(&mut BTreeMap<Skill, ProficiencyLevel>),
    ) {
        f(&mut self.skills);
    }

    pub fn update_proficiencies(&mut self, f: impl FnOnce(&mut VecSet<Proficiency>)) {
        f(&mut self.proficiencies);
    }

    pub fn saving_throw_bonus(&self, ability: Ability) -> i32 {
        let modifier = self.ability_modifier(ability);
        let proficient = self.proficient_with(ability);
        modifier
            + if proficient {
                self.proficiency_bonus()
            } else {
                0
            }
    }

    pub fn skill_proficiency(&self, skill: Skill) -> ProficiencyLevel {
        self.skills
            .get(&skill)
            .copied()
            .unwrap_or(ProficiencyLevel::None)
    }

    pub fn skill_bonus(&self, skill: Skill) -> i32 {
        let ability = skill.ability();
        let modifier = self.ability_modifier(ability);
        let prof_level = self.skill_proficiency(skill);
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

    /// Reset all derived state for replay. Preserves identity (including
    /// applied flags), equipment, personality, notes, and feature list with
    /// sources intact.
    pub fn reset_computed(&mut self) {
        self.abilities = AbilityScores::default();
        self.saving_throws.clear();
        self.skills.clear();
        self.feature_data.clear();
        self.proficiencies.clear();
        self.languages.clear();
        self.damage_modifiers.clear();
        self.spell_slots.clear();
        self.combat = CombatStats::default();
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
                for spell in spells.known.iter_mut().flatten() {
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
            saving_throws: VecSet::new(),
            skills: BTreeMap::new(),
            combat: CombatStats::default(),
            personality: Personality::default(),
            features: Features::default(),
            equipment: Equipment::default(),
            feature_data: BTreeMap::new(),
            spell_slots: BTreeMap::new(),
            proficiencies: VecSet::new(),
            languages: VecSet::new(),
            damage_modifiers: BTreeMap::new(),
            notes: String::new(),
            updated_at: now_epoch_secs(),
            shared: false,
        }
    }
}

fn set_damage_flag(
    map: &mut BTreeMap<DamageType, DamageModifiers>,
    dt: DamageType,
    value: i32,
    field: impl FnOnce(&mut DamageModifiers) -> &mut bool,
) {
    let entry = map.entry(dt).or_default();
    *field(entry) = value != 0;
    if !entry.is_active() {
        map.remove(&dt);
    }
}

impl expr::Context<Attribute, i32> for Character {
    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        match var {
            Attribute::Ability(ability) => {
                self.abilities.set(ability, value.max(1) as u32);
            }
            Attribute::MaxHp => {
                self.combat.hp_max = value as u32;
            }
            Attribute::Hp => {
                self.combat.hp_current = value as u32;
            }
            Attribute::TempHp => {
                self.combat.hp_temp = value as u32;
            }
            Attribute::Ac => {
                self.combat.armor_class = value as u32;
            }
            Attribute::Speed => {
                self.combat.speed = value as u32;
            }
            Attribute::AttackBonus => {
                self.combat.attack_bonus = value;
            }
            Attribute::Attacks => {
                self.combat.attack_count = value.max(1) as u32;
            }
            Attribute::InitiativeBonus => {
                self.combat.initiative_misc_bonus = value;
            }
            Attribute::SkillProficiency(skill) => {
                let level = match value.clamp(0, 2) {
                    0 => ProficiencyLevel::None,
                    1 => ProficiencyLevel::Proficient,
                    _ => ProficiencyLevel::Expertise,
                };
                self.update_skill_proficiencies(|skills| {
                    skills.insert(skill, level);
                });
            }
            Attribute::SaveProficiency(ability) => {
                self.update_saving_throw_proficiencies(|saves| {
                    if value != 0 {
                        saves.insert(ability);
                    } else {
                        saves.remove(&ability);
                    }
                });
            }
            Attribute::EquipmentProficiency(prof) => {
                if value != 0 {
                    self.proficiencies.insert(prof);
                } else {
                    self.proficiencies.remove(&prof);
                }
            }
            Attribute::Inspiration => {
                self.combat.inspiration = value != 0;
            }
            Attribute::Resistance(dt) => {
                set_damage_flag(&mut self.damage_modifiers, dt, value, |m| &mut m.resistant);
            }
            Attribute::Vulnerability(dt) => {
                set_damage_flag(&mut self.damage_modifiers, dt, value, |m| &mut m.vulnerable);
            }
            Attribute::Immunity(dt) => {
                set_damage_flag(&mut self.damage_modifiers, dt, value, |m| &mut m.immune);
            }
            Attribute::DamageReduction(dt) => {
                let entry = self.damage_modifiers.entry(dt).or_default();
                entry.reduction = value.max(0) as u32;
                if !entry.is_active() {
                    self.damage_modifiers.remove(&dt);
                }
            }
            other => return Err(expr::Error::read_only_var(other)),
        }

        Ok(())
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::Ability(ability) => Ok(self.abilities.get(ability) as i32),
            Attribute::Modifier(ability) => Ok(self.abilities.modifier(ability)),
            Attribute::SavingThrow(ability) => Ok(self.saving_throw_bonus(ability)),
            Attribute::Skill(skill) => Ok(self.skill_bonus(skill)),
            Attribute::SkillProficiency(skill) => Ok(self.skill_proficiency(skill).multiplier()),
            Attribute::SaveProficiency(ability) => Ok(self.proficient_with(ability) as i32),
            Attribute::EquipmentProficiency(prof) => Ok(self.proficiencies.contains(&prof) as i32),
            Attribute::MaxHp => Ok(self.combat.hp_max as i32),
            Attribute::Hp => Ok(self.combat.hp_current as i32),
            Attribute::TempHp => Ok(self.combat.hp_temp as i32),
            Attribute::Level => Ok(self.level() as i32),
            Attribute::Ac => Ok(self.combat.armor_class as i32),
            Attribute::Speed => Ok(self.combat.speed as i32),
            Attribute::CasterLevel(None) => Ok(self
                .caster_level(SpellSlotPool::Arcane)
                .max(self.caster_level(SpellSlotPool::Pact))
                as i32),
            Attribute::CasterLevel(Some(pool)) => Ok(self.caster_level(pool) as i32),
            Attribute::ProfBonus => Ok(self.proficiency_bonus()),
            Attribute::AttackBonus => Ok(self.combat.attack_bonus),
            Attribute::Attacks => Ok(self.combat.attack_count as i32),
            Attribute::Initiative => Ok(self.initiative()),
            Attribute::InitiativeBonus => Ok(self.combat.initiative_misc_bonus),
            Attribute::Inspiration => Ok(self.combat.inspiration as i32),
            Attribute::Resistance(dt) => {
                Ok(self.damage_modifiers.get(&dt).is_some_and(|m| m.resistant) as i32)
            }
            Attribute::Vulnerability(dt) => {
                Ok(self.damage_modifiers.get(&dt).is_some_and(|m| m.vulnerable) as i32)
            }
            Attribute::Immunity(dt) => {
                Ok(self.damage_modifiers.get(&dt).is_some_and(|m| m.immune) as i32)
            }
            Attribute::DamageReduction(dt) => Ok(self
                .damage_modifiers
                .get(&dt)
                .map_or(0, |m| m.reduction as i32)),
            a if a.is_advantage() => Ok(0),
            other => Err(expr::Error::unsupported_var(other)),
        }
    }
}

pub struct Context<'a> {
    pub character: &'a mut Character,
    pub class_level: i32,
    pub caster_level: i32,
    pub caster_modifier: i32,
    /// Extracted Points/Die field values: (field_index, available, max).
    /// Populated from FeatureData before expression evaluation, written back
    /// after.
    pub points: Vec<(u8, i32, i32)>,
}

impl<'a> From<&'a mut Character> for Context<'a> {
    fn from(character: &'a mut Character) -> Self {
        Self {
            character,
            class_level: 0,
            caster_level: 0,
            caster_modifier: 0,
            points: Vec::new(),
        }
    }
}

impl Context<'_> {
    /// Extract (available, max) from Points/Die fields at their actual indices.
    pub fn extract_points(feature_data: &FeatureData) -> Vec<(u8, i32, i32)> {
        feature_data
            .fields
            .iter()
            .enumerate()
            .filter_map(|(idx, field)| match &field.value {
                FeatureValue::Points { used, max } => {
                    Some((idx as u8, (*max - *used) as i32, *max as i32))
                }
                FeatureValue::Die { die, used } => {
                    Some((idx as u8, (die.amount - *used) as i32, die.amount as i32))
                }
                _ => None,
            })
            .collect()
    }

    /// Write back modified points values into the feature data fields.
    pub fn writeback_points(feature_data: &mut FeatureData, points: &[(u8, i32, i32)]) {
        for &(idx, available, max) in points {
            let Some(field) = feature_data.fields.get_mut(idx as usize) else {
                continue;
            };
            match &mut field.value {
                FeatureValue::Points { used, .. } => {
                    *used = (max - available).max(0) as u32;
                }
                FeatureValue::Die { used, .. } => {
                    *used = (max - available).max(0) as u32;
                }
                _ => {}
            }
        }
    }

    fn resolve_points(&self, idx: u8) -> Result<i32, expr::Error> {
        self.points
            .iter()
            .find(|(i, _, _)| *i == idx)
            .map(|(_, available, _)| *available)
            .ok_or(expr::Error::unsupported_var(Attribute::Points(idx)))
    }

    fn resolve_points_max(&self, idx: u8) -> Result<i32, expr::Error> {
        self.points
            .iter()
            .find(|(i, _, _)| *i == idx)
            .map(|(_, _, max)| *max)
            .ok_or(expr::Error::unsupported_var(Attribute::PointsMax(idx)))
    }

    fn assign_points(&mut self, idx: u8, value: i32) -> Result<(), expr::Error> {
        let entry = self
            .points
            .iter_mut()
            .find(|(i, _, _)| *i == idx)
            .ok_or(expr::Error::unsupported_var(Attribute::Points(idx)))?;
        entry.1 = value.clamp(0, entry.2);
        Ok(())
    }

    fn assign_points_max(&mut self, idx: u8, value: i32) -> Result<(), expr::Error> {
        let entry = self
            .points
            .iter_mut()
            .find(|(i, _, _)| *i == idx)
            .ok_or(expr::Error::unsupported_var(Attribute::PointsMax(idx)))?;
        entry.2 = value.max(0);
        Ok(())
    }
}

impl expr::Context<Attribute, i32> for Context<'_> {
    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        match var {
            Attribute::Points(n) => self.assign_points(n, value),
            Attribute::PointsMax(n) => self.assign_points_max(n, value),
            _ => self.character.assign(var, value),
        }
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        match var {
            Attribute::ClassLevel => Ok(self.class_level),
            Attribute::CasterLevel(None) => Ok(self.caster_level),
            Attribute::CasterLevel(Some(pool)) => Ok(self.character.caster_level(pool) as i32),
            Attribute::CasterModifier => Ok(self.caster_modifier),
            Attribute::Points(n) => self.resolve_points(n),
            Attribute::PointsMax(n) => self.resolve_points_max(n),
            _ => self.character.resolve(var),
        }
    }
}

#[cfg(test)]
impl Character {
    pub fn test_character() -> Character {
        use crate::model::{ClassLevel, FeatureSource, Spell, SpellData};

        let mut ch = Character {
            id: Uuid::nil(),
            identity: CharacterIdentity {
                name: "Share Test".to_string(),
                classes: vec![ClassLevel {
                    class: "Bard".to_string(),
                    class_label: None,
                    subclass: None,
                    subclass_label: None,
                    level: 3,
                    hit_die_sides: 8,
                    hit_dice_used: 0,
                    applied_levels: VecSet::new(),
                }],
                species: "Elf".to_string(),
                background: "Entertainer".to_string(),
                alignment: Alignment::ChaoticGood,
                experience_points: 900,
                species_applied: true,
                background_applied: true,
            },
            abilities: AbilityScores {
                strength: 8,
                dexterity: 14,
                constitution: 12,
                intelligence: 10,
                wisdom: 13,
                charisma: 16,
            },
            saving_throws: [Ability::Dexterity, Ability::Charisma]
                .into_iter()
                .collect(),
            skills: BTreeMap::new(),
            combat: CombatStats {
                armor_class: 13,
                speed: 30,
                hp_max: 24,
                hp_current: 20,
                hp_temp: 5,
                death_save_successes: 2,
                death_save_failures: 1,
                attack_bonus: 0,
                initiative_misc_bonus: 0,
                inspiration: false,
                attack_count: 1,
            },
            personality: Personality::default(),
            features: vec![Feature {
                name: "Bardic Inspiration".to_string(),
                label: None,
                description: "Use a bonus action...".to_string(),
                applied: true,
                source: FeatureSource::Class("Bard".to_string(), 1),
                inputs: Vec::new(),
            }]
            .into(),
            equipment: Equipment::default(),
            feature_data: BTreeMap::from([(
                "Spellcasting (Bard)".to_string(),
                FeatureData {
                    fields: Vec::new(),
                    spells: Some(SpellData {
                        casting_ability: Ability::Charisma,
                        caster_coef: 1,
                        pool: SpellSlotPool::Arcane,
                        spells: vec![Spell {
                            name: "Vicious Mockery".to_string(),
                            label: None,
                            level: 0,
                            description: "Unleash a string of insults...".to_string(),
                            sticky: false,
                            cost: 0,
                            free_uses: None,
                        }],
                        known: None,
                    }),
                },
            )]),
            proficiencies: VecSet::new(),
            languages: VecSet::new(),
            damage_modifiers: BTreeMap::new(),
            spell_slots: BTreeMap::new(),
            notes: String::new(),
            updated_at: 0,
            shared: false,
        };
        ch.update_spell_slots(SpellSlotPool::Arcane, None);
        ch
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::BTreeMap;

    use wasm_bindgen_test::*;

    use super::*;
    use crate::{
        expr::Expr,
        model::{Armor, ClassLevel, Currency, FeatureSource, Money, SpellData},
        vecset::VecSet,
    };

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
                species: "Human".to_string(),
                background: "Soldier".to_string(),
                alignment: Alignment::TrueNeutral,
                experience_points: 0,
                species_applied: false,
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
            saving_throws: [Ability::Strength, Ability::Constitution]
                .into_iter()
                .collect(),
            skills: BTreeMap::from([
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
                attack_bonus: 0,
                initiative_misc_bonus: 0,
                inspiration: false,
                attack_count: 1,
            },
            personality: Personality::default(),
            features: Features::default(),
            equipment: Equipment::default(),
            feature_data: BTreeMap::new(),
            proficiencies: [
                Proficiency::LightArmor,
                Proficiency::MediumArmor,
                Proficiency::HeavyArmor,
                Proficiency::Shields,
            ]
            .into_iter()
            .collect(),
            languages: VecSet::new(),
            damage_modifiers: BTreeMap::new(),
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
        ch.features.push(Feature {
            name: feature_name.to_string(),
            source: FeatureSource::Class(class_name.to_string(), 1),
            applied: true,
            ..Default::default()
        });
        ch.feature_data.insert(
            feature_name.to_string(),
            FeatureData {
                spells: Some(SpellData {
                    casting_ability: Ability::Intelligence,
                    caster_coef,
                    pool,
                    spells: Vec::new(),
                    known: None,
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
        assert!(c.spend(Money::from_cp(50))); // 5 sp
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
        assert!(c.spend(Money::from_cp(50))); // 5 sp
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
        assert!(c.spend(Money::from_cp(30))); // 3 sp
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
        // 2 ep → spend 1 ep (50 cp) → 1 ep (exact match, no break needed)
        let mut c = Currency {
            ep: 2,
            sp: 3,
            ..Default::default()
        };
        assert!(c.spend(Money::from_cp(50))); // 1 ep
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
        assert!(c.spend(Money::from_cp(30))); // 3 sp
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
        assert!(c.spend(Money::from_cp(150))); // 15 sp
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
        // 2 pp → spend 1 pp (1000 cp) → 1 pp
        let mut c = Currency {
            pp: 2,
            ..Default::default()
        };
        assert!(c.spend(Money::from_cp(1000))); // 1 pp
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

    // --- compute_armor_class ---

    fn make_armor(name: &str, base_ac: u32, armor_type: ArmorType, expr_str: &str) -> Armor {
        Armor {
            name: name.to_string(),
            base_ac,
            armor_type,
            ac_expr: if expr_str.is_empty() {
                None
            } else {
                Some(expr_str.parse::<Expr<Attribute>>().unwrap())
            },
        }
    }

    #[wasm_bindgen_test]
    fn computed_ac_no_armor() {
        // DEX 14 → modifier +2 → 10 + 2 = 12
        let mut ch = test_character();
        ch.equipment.armors.clear();
        let ac = ch.compute_armor_class();
        assert_eq!(ac, 12);
    }

    #[wasm_bindgen_test]
    fn computed_ac_light_armor() {
        // Leather: 11 + DEX.MOD(+2) = 13
        let mut ch = test_character();
        ch.equipment.armors = vec![make_armor("Leather", 11, ArmorType::Light, "11 + DEX.MOD")];
        let ac = ch.compute_armor_class();
        assert_eq!(ac, 13);
    }

    #[wasm_bindgen_test]
    fn computed_ac_medium_armor() {
        // Chain shirt: 13 + min(DEX.MOD(+2), 2) = 15
        let mut ch = test_character();
        ch.equipment.armors = vec![make_armor(
            "Chain Shirt",
            13,
            ArmorType::Medium,
            "13 + min(DEX.MOD, 2)",
        )];
        let ac = ch.compute_armor_class();
        assert_eq!(ac, 15);
    }

    #[wasm_bindgen_test]
    fn computed_ac_heavy_armor() {
        // Plate: 18
        let mut ch = test_character();
        ch.equipment.armors = vec![make_armor("Plate", 18, ArmorType::Heavy, "18")];
        let ac = ch.compute_armor_class();
        assert_eq!(ac, 18);
    }

    #[wasm_bindgen_test]
    fn computed_ac_with_shield() {
        // Plate(18) + Shield(+2) = 20
        let mut ch = test_character();
        ch.equipment.armors = vec![
            make_armor("Plate", 18, ArmorType::Heavy, "18"),
            make_armor("Shield", 2, ArmorType::Shield, "AC + 2"),
        ];
        let ac = ch.compute_armor_class();
        assert_eq!(ac, 20);
    }

    #[wasm_bindgen_test]
    fn computed_ac_natural_armor() {
        // Unarmored Defense (Barbarian): 10 + DEX(+2) + CON(+1) = 13
        let mut ch = test_character();
        ch.equipment.armors = vec![make_armor(
            "Unarmored Defense",
            0,
            ArmorType::Natural,
            "10 + DEX.MOD + CON.MOD",
        )];
        let ac = ch.compute_armor_class();
        assert_eq!(ac, 13);
    }

    #[wasm_bindgen_test]
    fn computed_ac_picks_best() {
        // Leather(13) vs Plate(18) vs Natural(13) → picks 18
        let mut ch = test_character();
        ch.equipment.armors = vec![
            make_armor("Leather", 11, ArmorType::Light, "11 + DEX.MOD"),
            make_armor("Plate", 18, ArmorType::Heavy, "18"),
            make_armor(
                "Unarmored Defense",
                0,
                ArmorType::Natural,
                "10 + DEX.MOD + CON.MOD",
            ),
        ];
        let ac = ch.compute_armor_class();
        assert_eq!(ac, 18);
    }

    // --- compute_hp_max ---

    #[wasm_bindgen_test]
    fn compute_hp_max_single_class() {
        // Fighter level 5, d10, CON 12 (mod +1)
        // base = 10 + 4 * avg_hp(10) = 10 + 4 * 6 = 34
        // con = 5 * 1 = 5
        // total = 39
        let mut ch = test_character();
        let hp = ch.compute_hp_max();
        assert_eq!(hp, 39);
        assert_eq!(ch.combat.hp_max, 39);
    }

    #[wasm_bindgen_test]
    fn compute_hp_max_multiclass() {
        // Fighter 5 (d10) + Wizard 2 (d6), CON 12 (mod +1), total level 7
        // Fighter: 10 + 4 * 6 = 34
        // Wizard: 6 + 1 * 4 = 10
        // con = 7 * 1 = 7
        // total = 51
        let mut ch = test_character();
        ch.identity.classes.push(ClassLevel {
            class: "Wizard".to_string(),
            class_label: None,
            subclass: None,
            subclass_label: None,
            level: 2,
            hit_die_sides: 6,
            hit_dice_used: 0,
            applied_levels: VecSet::new(),
        });
        let hp = ch.compute_hp_max();
        assert_eq!(hp, 51);
    }

    #[wasm_bindgen_test]
    fn compute_hp_max_negative_con() {
        // Fighter level 5, d10, CON 6 (mod -2)
        // base = 10 + 4 * 6 = 34
        // con = 5 * (-2) = -10
        // total = 24
        let mut ch = test_character();
        ch.abilities.constitution = 6;
        let hp = ch.compute_hp_max();
        assert_eq!(hp, 24);
    }

    #[wasm_bindgen_test]
    fn compute_speed_resets_to_default() {
        let mut ch = test_character();
        ch.combat.speed = 50;
        let speed = ch.compute_speed();
        assert_eq!(speed, 30);
        assert_eq!(ch.combat.speed, 30);
    }
}
