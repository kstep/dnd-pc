use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    expr::{self, Eval as _},
    model::{
        AbilityScores, Applied, Attribute, CharacterIdentity, CombatStats, DamageModifiers,
        Equipment, Feature, FeatureData, FeatureSource, FeatureValue, Features, Note, Personality,
        Skills, SpellSlots, Weapon, enums::*,
    },
    vecset::VecSet,
};

/// Default walking speed in feet (most species).
const DEFAULT_SPEED: u32 = 30;

/// Maximum class level a user can enter. D&D 5e standard progression caps at
/// 20; we allow up to 40 for epic-tier campaigns and homebrew content. Tables
/// like `XP_THRESHOLDS` and `SPELL_SLOT_TABLE` only cover 1–20 — levels above
/// 20 reuse the level-20 row for spell slots and report a 0 XP threshold.
pub const MAX_CLASS_LEVEL: u32 = 40;

/// Why the character requires a `rebuild()`. Reported by
/// `Character::rebuild_reasons` so the UI can explain what specifically
/// drifted between `identity` and `applied`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebuildReason {
    SpeciesChanged,
    BackgroundChanged,
    ClassRemoved(String),
    LevelLowered {
        class: String,
        applied: u32,
        current: u32,
    },
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub id: Uuid,
    pub name: String,
    pub class: String,
    pub level: u32,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub avatar_updated_at: Option<u64>,
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
    abilities: AbilityScores,
    #[serde(default)]
    saving_throws: VecSet<Ability>,
    #[serde(default)]
    pub skills: Skills,
    #[serde(default)]
    pub combat: CombatStats,
    #[serde(default)]
    pub personality: Personality,
    #[serde(default)]
    pub features: Features,
    #[serde(default)]
    pub equipment: Equipment,
    #[serde(default)]
    pub proficiencies: VecSet<Proficiency>,
    #[serde(default)]
    pub languages: VecSet<String>,
    #[serde(default)]
    pub damage_modifiers: DamageModifiers,
    #[serde(default)]
    pub spell_slots: SpellSlots,
    #[serde(default)]
    pub applied: Applied,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub shared: bool,
    pub schema_version: u32,
}

pub fn now_epoch_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

impl Character {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fresh character carrying only the given identity — every other field
    /// starts from `Default`. Used by rebuild's `build_clean` as the accretion
    /// target and by the rebuild args-modal as the cascade base.
    pub fn from_identity(identity: CharacterIdentity) -> Self {
        Self {
            identity,
            ..Self::default()
        }
    }

    pub fn clear(&mut self) {
        let id = self.id;
        let identity = std::mem::take(&mut self.identity);
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
        self.combat.concentrating = None;

        for cl in &mut self.identity.classes {
            cl.hit_dice_used = 0;
        }

        self.spell_slots.reset_used();
        self.features.reset_uses();
    }

    pub fn short_rest(&mut self) {
        self.combat.death_save_failures = 0;
        self.combat.death_save_successes = 0;

        self.spell_slots
            .reset_used_where(SpellSlotPool::restore_on_short_rest);
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

    pub fn set_ability(&mut self, ability: Ability, value: u32) {
        self.abilities.set(ability, value.max(1));
    }

    /// Compare the derived state (what feature-apply produces) between two
    /// characters. Ignores identity (user input), build (features.list —
    /// rebuild restructures it), personality (untouched), and
    /// compute-derived fields (hp/ac/spell_slots re-derive after commit).
    pub fn eq_derived(&self, other: &Self) -> bool {
        self.abilities == other.abilities
            && self.saving_throws == other.saving_throws
            && self.skills == other.skills
            && self.proficiencies == other.proficiencies
            && self.languages == other.languages
            && self.damage_modifiers == other.damage_modifiers
    }

    pub fn features(&self) -> &[Feature] {
        &self.features.list
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

    /// Evaluate equipped armor AC formulas and apply if better than the
    /// current AC (which may already include natural armor from assignments).
    ///
    /// Expects `combat.armor_class` to be pre-set as the baseline
    /// (default `10 + DEX.MOD`, possibly overridden by OnCompute assignments).
    ///
    /// Evaluation order:
    /// 1. All non-shield, non-natural armor formulas → pick the max vs baseline
    /// 2. Set AC so shield formulas can read it
    /// 3. All shield formulas → pick the max
    pub fn compute_armor_class(&mut self) -> u32 {
        let baseline = self.combat.armor_class;

        // Best body armor (non-shield, non-natural), skipping armor the
        // character isn't proficient with. Natural armor AC comes through
        // OnCompute assignments, not through equipment evaluation.
        if let Some(body_ac) = self
            .equipment
            .armors
            .iter()
            .filter(|armor| {
                armor.armor_type != ArmorType::Shield && armor.armor_type != ArmorType::Natural
            })
            .filter(|armor| {
                armor
                    .armor_type
                    .required_proficiency()
                    .is_none_or(|prof| self.proficiencies.contains(&prof))
            })
            .filter_map(|armor| {
                let expr = armor.ac_expr.as_ref()?;
                match expr.eval(self) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        log::warn!("AC expr eval failed for '{}': {error}", armor.name);
                        None
                    }
                }
            })
            .max()
            .map(|ac| ac.max(0) as u32)
        {
            self.combat.armor_class = baseline.max(body_ac);
        }

        // Best shield (reads AC = body_ac), only if proficient with shields
        if !self.proficiencies.contains(&Proficiency::Shields) {
            return self.combat.armor_class;
        }
        if let Some(shield_ac) = self
            .equipment
            .armors
            .iter()
            .filter(|armor| armor.armor_type == ArmorType::Shield)
            .filter_map(|armor| {
                let expr = armor.ac_expr.as_ref()?;
                match expr.eval(self) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        log::warn!("AC expr eval failed for '{}': {error}", armor.name);
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

    /// Reset base combat stats to defaults before feature assignments.
    ///
    /// Sets default AC (`10 + DEX.MOD`), recomputes HP and speed,
    /// and resets misc bonuses. After this, call
    /// `RulesRegistry::assign(character, OnCompute)` to apply feature
    /// bonuses (including natural armor via `AC = max(AC, ...)`),
    /// then `compute_armor_class()` to apply equipped armor and shields.
    pub fn compute(&mut self) {
        self.combat.armor_class = (10 + self.ability_modifier(Ability::Dexterity)).max(0) as u32;
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
                    let spell_data = self.features.get(&feature.name)?.spells.as_ref()?;
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

    pub fn update_spell_slots(&mut self, pool: SpellSlotPool, slots: Option<&[u32]>) {
        let (caster_level, caster_classes) = self.caster_info(pool);
        let table_slots: &[u32] = caster_level
            .checked_sub(1)
            .and_then(|level| SPELL_SLOT_TABLE.get(level as usize))
            .copied()
            .unwrap_or(&[]);
        let effective: &[u32] = match caster_classes {
            0 => &[],
            1 => slots
                .filter(|override_slots| !override_slots.is_empty())
                .unwrap_or(table_slots),
            _ => table_slots,
        };

        self.spell_slots.set_totals(pool, effective);
    }

    pub fn can_level_up(&self) -> bool {
        !self.identity.classes.is_empty()
            && self.identity.classes.iter().all(|cl| !cl.class.is_empty())
    }

    /// True if there are forward-only changes that can be materialized
    /// without `rebuild()`: pending class levels (new class or new levels of
    /// an existing class), or species/background not yet applied while
    /// `features` is still empty (first-time application). Once any feature
    /// has been applied, species/background cannot be inserted ahead of it
    /// — that case routes through `needs_rebuild()` instead. Callers should
    /// check `needs_rebuild()` first; apply only makes sense when `applied`
    /// is a strict prefix of `identity`.
    pub fn has_pending_apply(&self) -> bool {
        let no_features = self.features.list.is_empty();
        let species_apply =
            !self.identity.species.is_empty() && !self.applied.species && no_features;
        let background_apply =
            !self.identity.background.is_empty() && !self.applied.background && no_features;
        species_apply
            || background_apply
            || self.identity.classes.iter().any(|cl| {
                !cl.class.is_empty()
                    && (1..=cl.level).any(|lvl| !self.applied.contains_level(&cl.class, lvl))
            })
    }

    /// Returns the list of drift reasons that require `rebuild()`. Empty
    /// when applied state matches identity. UI surfaces this list in the
    /// rebuild banner so the user knows what triggered it.
    pub fn rebuild_reasons(&self) -> Vec<RebuildReason> {
        let mut reasons = Vec::new();
        let has_features = !self.features.list.is_empty();

        if !self.identity.species.is_empty() && !self.applied.species && has_features {
            reasons.push(RebuildReason::SpeciesChanged);
        }
        if !self.identity.background.is_empty() && !self.applied.background && has_features {
            reasons.push(RebuildReason::BackgroundChanged);
        }
        // Applied levels for a class no longer present in identity (deleted or
        // renamed). Empty level sets are tolerated — they may be tombstones.
        for (class, lvls) in &self.applied.levels {
            if lvls.is_empty() {
                continue;
            }
            if !self.identity.classes.iter().any(|cl| &cl.class == class) {
                reasons.push(RebuildReason::ClassRemoved(class.clone()));
            }
        }
        // Class level lowered: applied retains levels above the current
        // identity level.
        for cl in &self.identity.classes {
            if cl.class.is_empty() {
                continue;
            }
            if let Some(lvls) = self.applied.levels.get(&cl.class)
                && let Some(&max) = lvls.iter().max()
                && max > cl.level
            {
                reasons.push(RebuildReason::LevelLowered {
                    class: cl.class.clone(),
                    applied: max,
                    current: cl.level,
                });
            }
        }
        reasons
    }

    /// True if `applied` references slots that no longer match `identity`.
    /// See `rebuild_reasons` for the breakdown.
    pub fn needs_rebuild(&self) -> bool {
        !self.rebuild_reasons().is_empty()
    }

    pub fn level(&self) -> u32 {
        self.identity
            .classes
            .iter()
            .map(|cl| cl.level)
            .sum::<u32>()
            .max(1)
    }

    /// Effective current level for a feature based on its source.
    /// Class features use their class's current level; others use total level.
    pub fn effective_level_for(&self, source: &FeatureSource) -> u32 {
        match source {
            FeatureSource::Class(class_name, _) | FeatureSource::Subclass(class_name, _, _) => self
                .identity
                .classes
                .iter()
                .find(|cl| cl.class.as_str() == &**class_name)
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

    pub fn skill_bonus(&self, skill: Skill) -> i32 {
        let ability = skill.ability();
        let modifier = self.ability_modifier(ability);
        let prof_level = self.skills.get(skill);
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

    /// Evaluate a weapon's attack-bonus expression against this character.
    /// Uses the weapon's explicit `attack_expr` when set, otherwise the
    /// default derived from its `category` / `ability` / `magic_bonus`.
    pub fn weapon_attack_bonus(&self, weapon: &Weapon) -> i32 {
        weapon.effective_attack_expr().eval(self).unwrap_or(0)
    }

    /// Clone containing only fields that participate in Expr analysis and
    /// `compute`. Large free-text fields (`personality`, `notes`) are skipped,
    /// and `equipment` keeps only `weapons` + `armors` (their exprs feed AC /
    /// attack evaluation); inventory `items` and `currency` are dropped as
    /// decorative. Used for transient cascade bases in the args modal where
    /// the clone is read-only and discarded on modal close.
    pub fn clone_lean(&self) -> Self {
        let equipment = Equipment {
            weapons: self.equipment.weapons.clone(),
            armors: self.equipment.armors.clone(),
            ..Equipment::default()
        };
        Self {
            id: self.id,
            identity: self.identity.clone(),
            abilities: self.abilities,
            saving_throws: self.saving_throws.clone(),
            skills: self.skills.clone(),
            combat: self.combat.clone(),
            features: self.features.clone(),
            equipment,
            proficiencies: self.proficiencies.clone(),
            languages: self.languages.clone(),
            damage_modifiers: self.damage_modifiers.clone(),
            spell_slots: self.spell_slots.clone(),
            applied: self.applied.clone(),
            updated_at: self.updated_at,
            shared: self.shared,
            schema_version: self.schema_version,
            ..Self::default()
        }
    }

    /// Reset all derived state for replay. Preserves identity (including
    /// applied flags), equipment, personality, notes, and feature list with
    /// sources intact.
    pub fn reset_computed(&mut self) {
        self.abilities = AbilityScores::default();
        self.saving_throws.clear();
        self.skills.clear();
        self.features.clear();
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
        self.features.clear_all_labels();
    }

    pub fn class_summary(&self) -> String {
        crate::model::format_classes(&self.identity.classes)
    }
}

impl Default for Character {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            identity: CharacterIdentity::default(),
            abilities: AbilityScores::default(),
            saving_throws: VecSet::new(),
            skills: Skills::default(),
            combat: CombatStats::default(),
            personality: Personality::default(),
            features: Features::default(),
            equipment: Equipment::default(),
            spell_slots: SpellSlots::default(),
            applied: Applied::default(),
            proficiencies: VecSet::new(),
            languages: VecSet::new(),
            damage_modifiers: DamageModifiers::default(),
            notes: Vec::new(),
            updated_at: now_epoch_secs(),
            shared: false,
            schema_version: 1,
        }
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
                self.skills.set(skill, level);
            }
            Attribute::SaveProficiency(ability) => {
                if value != 0 {
                    self.saving_throws.insert(ability);
                } else {
                    self.saving_throws.remove(&ability);
                }
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
            Attribute::Language(name) => {
                if value != 0 {
                    self.languages.insert(name.to_string());
                } else {
                    self.languages.remove(name);
                }
            }
            Attribute::Resistance(dt) => {
                self.damage_modifiers.set_resistant(dt, value != 0);
            }
            Attribute::Vulnerability(dt) => {
                self.damage_modifiers.set_vulnerable(dt, value != 0);
            }
            Attribute::Immunity(dt) => {
                self.damage_modifiers.set_immune(dt, value != 0);
            }
            Attribute::DamageReduction(dt) => {
                self.damage_modifiers.set_reduction(dt, value.max(0) as u32);
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
            Attribute::SkillProficiency(skill) => Ok(self.skills.get(skill).multiplier()),
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
            Attribute::Resistance(dt) => Ok(self.damage_modifiers.is_resistant(dt) as i32),
            Attribute::Vulnerability(dt) => Ok(self.damage_modifiers.is_vulnerable(dt) as i32),
            Attribute::Immunity(dt) => Ok(self.damage_modifiers.is_immune(dt) as i32),
            Attribute::DamageReduction(dt) => Ok(self.damage_modifiers.reduction(dt) as i32),
            Attribute::Feature(name) => Ok(self.features.has(name) as i32),
            Attribute::Language(name) => Ok(self.languages.contains(name) as i32),
            Attribute::FeatCategory(cat) => Ok(self.features.has_category(cat) as i32),
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
        use std::collections::BTreeMap;

        use crate::model::{ClassLevel, FeatureCategory, FeatureSource, Spell, SpellData};

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
                }],
                species: "Elf".to_string(),
                background: "Entertainer".to_string(),
                experience_points: 900,
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
            skills: Skills::default(),
            combat: CombatStats {
                concentrating: None,
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
            personality: Personality {
                alignment: Alignment::ChaoticGood,
                ..Personality::default()
            },
            features: Features::from_parts(
                vec![Feature {
                    name: "Bardic Inspiration".to_string(),
                    label: None,
                    description: "Use a bonus action...".to_string(),
                    applied: true,
                    category: FeatureCategory::Class,
                    source: FeatureSource::Class("Bard".into(), 1),
                    inputs: Vec::new(),
                }]
                .into(),
                BTreeMap::from([(
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
            ),
            equipment: Equipment::default(),
            proficiencies: VecSet::new(),
            languages: VecSet::new(),
            damage_modifiers: DamageModifiers::default(),
            spell_slots: SpellSlots::default(),
            applied: Applied {
                species: true,
                background: true,
                levels: BTreeMap::new(),
            },
            notes: Vec::new(),
            updated_at: 0,
            shared: false,
            schema_version: 0,
        };
        ch.update_spell_slots(SpellSlotPool::Arcane, None);
        ch
    }
}

#[cfg(test)]
pub mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::{
        model::{Armor, ClassLevel, Currency, Expr, Feature, FeatureSource, Money, SpellData},
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
                }],
                species: "Human".to_string(),
                background: "Soldier".to_string(),
                experience_points: 0,
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
            skills: [
                (Skill::Athletics, ProficiencyLevel::Proficient),
                (Skill::Perception, ProficiencyLevel::Expertise),
            ]
            .into_iter()
            .collect(),
            combat: CombatStats {
                concentrating: None,
                armor_class: 12,
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
            proficiencies: [
                Proficiency::LightArmor,
                Proficiency::MediumArmor,
                Proficiency::HeavyArmor,
                Proficiency::Shields,
            ]
            .into_iter()
            .collect(),
            languages: VecSet::new(),
            damage_modifiers: DamageModifiers::default(),
            spell_slots: SpellSlots::default(),
            applied: Applied::default(),
            notes: Vec::new(),
            updated_at: 0,
            shared: false,
            schema_version: 0,
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
        ch.features.list.push(Feature {
            name: feature_name.to_string(),
            source: FeatureSource::Class(class_name.into(), 1),
            applied: true,
            ..Default::default()
        });
        ch.features.insert(
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
                Some(expr_str.parse::<Expr>().unwrap())
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
        // Natural armor is now applied via assign() OnCompute, which sets
        // combat.armor_class directly. compute_armor_class() skips Natural
        // type and uses the baseline. Simulate by setting baseline to 13.
        let mut ch = test_character();
        ch.combat.armor_class = 13; // as if assign() set 10 + DEX(+2) + CON(+1)
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

    // --- has_pending_apply / needs_rebuild ---

    fn drift_character() -> Character {
        let mut ch = test_character();
        ch.identity.classes.clear();
        ch.identity.species.clear();
        ch.identity.background.clear();
        ch.applied = Applied::default();
        ch
    }

    #[wasm_bindgen_test]
    fn drift_fresh_character_is_consistent() {
        let ch = drift_character();
        assert!(!ch.has_pending_apply());
        assert!(!ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_first_species_apply_no_features_yet() {
        // Fresh character picked species — features list still empty, so
        // species can be applied forward without rebuild.
        let mut ch = drift_character();
        ch.identity.species = "Elf".to_string();
        assert!(ch.has_pending_apply());
        assert!(!ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_first_background_apply_no_features_yet() {
        let mut ch = drift_character();
        ch.identity.background = "Sage".to_string();
        assert!(ch.has_pending_apply());
        assert!(!ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_species_changed_after_apply_needs_rebuild() {
        // Character already has applied class features; user changes species
        // → species_field reset applied.species=false. Forward apply would
        // insert species after class features (wrong order) — rebuild required.
        let mut ch = drift_character();
        ch.identity.species = "Elf".to_string();
        ch.identity.background = "Sage".to_string();
        ch.applied.background = true;
        ch.identity.classes.push(ClassLevel {
            class: "Wizard".to_string(),
            level: 1,
            ..ClassLevel::default()
        });
        ch.applied.mark_level("Wizard", 1);
        ch.features.list.push(Feature {
            name: "Magic Missile".to_string(),
            source: FeatureSource::Class("Wizard".into(), 1),
            applied: true,
            ..Default::default()
        });
        // applied.species = false (reset by species_field on rename).
        assert!(ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_background_changed_after_apply_needs_rebuild() {
        let mut ch = drift_character();
        ch.identity.species = "Elf".to_string();
        ch.identity.background = "Sage".to_string();
        ch.applied.species = true;
        ch.identity.classes.push(ClassLevel {
            class: "Wizard".to_string(),
            level: 1,
            ..ClassLevel::default()
        });
        ch.applied.mark_level("Wizard", 1);
        ch.features.list.push(Feature {
            name: "Magic Missile".to_string(),
            source: FeatureSource::Class("Wizard".into(), 1),
            applied: true,
            ..Default::default()
        });
        assert!(ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_pending_levels_existing_class() {
        let mut ch = drift_character();
        ch.identity.species = "Human".to_string();
        ch.identity.background = "Soldier".to_string();
        ch.applied.species = true;
        ch.applied.background = true;
        ch.identity.classes.push(ClassLevel {
            class: "Wizard".to_string(),
            level: 5,
            ..ClassLevel::default()
        });
        ch.applied.mark_level("Wizard", 1);
        ch.applied.mark_level("Wizard", 2);
        ch.applied.mark_level("Wizard", 3);
        assert!(ch.has_pending_apply());
        assert!(!ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_new_class_no_applied_entry() {
        let mut ch = drift_character();
        ch.identity.species = "Human".to_string();
        ch.identity.background = "Soldier".to_string();
        ch.applied.species = true;
        ch.applied.background = true;
        ch.identity.classes.push(ClassLevel {
            class: "Cleric".to_string(),
            level: 1,
            ..ClassLevel::default()
        });
        assert!(ch.has_pending_apply());
        assert!(!ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_class_removed_needs_rebuild() {
        let mut ch = drift_character();
        ch.identity.species = "Human".to_string();
        ch.identity.background = "Soldier".to_string();
        ch.applied.species = true;
        ch.applied.background = true;
        // Identity has no classes, but applied still tracks Wizard.
        ch.applied.mark_level("Wizard", 1);
        ch.applied.mark_level("Wizard", 2);
        assert!(ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_class_level_lowered_needs_rebuild() {
        let mut ch = drift_character();
        ch.identity.species = "Human".to_string();
        ch.identity.background = "Soldier".to_string();
        ch.applied.species = true;
        ch.applied.background = true;
        ch.identity.classes.push(ClassLevel {
            class: "Wizard".to_string(),
            level: 3,
            ..ClassLevel::default()
        });
        for lvl in 1..=5 {
            ch.applied.mark_level("Wizard", lvl);
        }
        assert!(ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_empty_class_name_ignored() {
        let mut ch = drift_character();
        ch.identity.species = "Human".to_string();
        ch.identity.background = "Soldier".to_string();
        ch.applied.species = true;
        ch.applied.background = true;
        // Pristine ClassLevel with empty class name (the add_class entry).
        ch.identity.classes.push(ClassLevel::default());
        assert!(!ch.has_pending_apply());
        assert!(!ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_multiclass_one_class_pending_other_applied() {
        let mut ch = drift_character();
        ch.identity.species = "Human".to_string();
        ch.identity.background = "Soldier".to_string();
        ch.applied.species = true;
        ch.applied.background = true;
        ch.identity.classes.push(ClassLevel {
            class: "Wizard".to_string(),
            level: 3,
            ..ClassLevel::default()
        });
        ch.identity.classes.push(ClassLevel {
            class: "Cleric".to_string(),
            level: 2,
            ..ClassLevel::default()
        });
        // Wizard fully applied, Cleric brand-new.
        for lvl in 1..=3 {
            ch.applied.mark_level("Wizard", lvl);
        }
        assert!(ch.has_pending_apply());
        assert!(!ch.needs_rebuild());
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
