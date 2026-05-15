use std::{
    mem,
    ops::{Deref, DerefMut},
};

use leptos_fluent::{I18n, tr};
use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    expr::{self, Eval as _},
    model::{
        AbilityScores, Applied, AttrKey, Attribute, CharacterIdentity, ClassLevel, CombatStats,
        DamageModifiers, Equipment, Feature, FeatureCategory, FeatureSource, Features,
        IdentitySlot, Note, Personality, Skills, SpellData, SpellSlots, ToolEntry, Tools, Weapon,
        enums::*,
    },
    vecset::VecSet,
};

/// Default walking speed in feet (most species).
const DEFAULT_SPEED: u32 = 30;

/// Maximum class level a user can enter. D&D 5e standard progression caps at
/// 20; we allow up to 40 for epic-tier campaigns and homebrew content.
/// `XP_THRESHOLDS` only covers 1–20 — levels above 20 report a 0 XP
/// threshold; spell slots come from per-feature `assign` expressions.
pub const MAX_CLASS_LEVEL: u32 = 40;

/// Why the character requires a `rebuild()`. Reported by
/// `Character::rebuild_reasons` so the UI can explain what specifically
/// drifted between `identity` and `applied`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebuildReason {
    SpeciesChanged,
    BackgroundChanged,
    ClassRemoved(Box<str>),
    LevelLowered {
        class: Box<str>,
        applied: u32,
        current: u32,
    },
    SubclassChanged {
        class: Box<str>,
    },
    FeatureRemoved(Box<str>),
    /// Built character with no `System(Class)` markers — needs the
    /// rebuild flow to emit them before further drift detection works.
    LegacyMissingSystemMarkers,
}

impl RebuildReason {
    pub fn label(&self, i18n: I18n) -> String {
        match self {
            Self::SpeciesChanged => tr!(i18n, "rebuild-reason-species"),
            Self::BackgroundChanged => tr!(i18n, "rebuild-reason-background"),
            Self::ClassRemoved(class) => {
                tr!(i18n, "rebuild-reason-class-removed", { "class" => class.to_string() })
            }
            Self::LevelLowered {
                class,
                applied,
                current,
            } => tr!(i18n, "rebuild-reason-level-lowered", {
                "class" => class.to_string(),
                "applied" => *applied,
                "current" => *current,
            }),
            Self::SubclassChanged { class } => {
                tr!(i18n, "rebuild-reason-subclass-changed", { "class" => class.to_string() })
            }
            Self::FeatureRemoved(name) => {
                tr!(i18n, "rebuild-reason-feature-removed", { "name" => name.to_string() })
            }
            Self::LegacyMissingSystemMarkers => tr!(i18n, "rebuild-reason-legacy-system-markers"),
        }
    }
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

// --- Character core ---

/// The slice of state the apply pipeline reads and writes — every field
/// `cascade()` / `apply_pending` / `compute()` touches lives here. A clone
/// of this is the speculative cascade snapshot in args modal flows.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Store)]
pub struct CharacterCore {
    #[serde(default)]
    pub identity: CharacterIdentity,
    #[serde(default)]
    pub abilities: AbilityScores,
    #[serde(default)]
    pub saving_throws: VecSet<Ability>,
    #[serde(default)]
    pub skills: Skills,
    #[serde(default)]
    pub combat: CombatStats,
    #[serde(default)]
    pub features: Features,
    #[serde(default)]
    pub proficiencies: VecSet<Proficiency>,
    #[serde(default)]
    pub languages: VecSet<String>,
    #[serde(default)]
    pub tools: Tools,
    #[serde(default)]
    pub damage_modifiers: DamageModifiers,
    #[serde(default)]
    pub spell_slots: SpellSlots,
    #[serde(default)]
    pub applied: Applied,
}

// --- Main Character ---

#[derive(Debug, Clone, Serialize, Deserialize, Store)]
pub struct Character {
    pub id: Uuid,
    #[serde(default)]
    pub core: CharacterCore,
    #[serde(default)]
    pub equipment: Equipment,
    #[serde(default)]
    pub personality: Personality,
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub shared: bool,
    pub schema_version: u32,
}

impl Deref for Character {
    type Target = CharacterCore;

    fn deref(&self) -> &CharacterCore {
        &self.core
    }
}

impl DerefMut for Character {
    fn deref_mut(&mut self) -> &mut CharacterCore {
        &mut self.core
    }
}

impl AsRef<CharacterCore> for CharacterCore {
    fn as_ref(&self) -> &CharacterCore {
        self
    }
}

impl AsRef<CharacterCore> for Character {
    fn as_ref(&self) -> &CharacterCore {
        &self.core
    }
}

pub fn now_epoch_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

impl CharacterCore {
    pub fn level(&self) -> u32 {
        self.identity
            .classes
            .iter()
            .filter(|class_level| !class_level.class.is_empty())
            .map(|class_level| class_level.level)
            .sum::<u32>()
    }

    /// Effective current level for a feature based on its source.
    /// Class features use their class's current level; others use total level.
    pub fn effective_level_for(&self, source: &FeatureSource) -> u32 {
        match source {
            FeatureSource::Class(class_name, _) | FeatureSource::Subclass(class_name, _, _) => self
                .identity
                .classes
                .iter()
                .find(|cl| cl.class.as_ref() == &**class_name)
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

    pub fn tool_level(&self, name: &str) -> ProficiencyLevel {
        self.tools.level(name)
    }

    pub fn tools(&self) -> impl Iterator<Item = (&ToolEntry, i32)> + '_ {
        let pb = self.proficiency_bonus();
        self.tools
            .iter()
            .filter(|entry| !entry.name.is_empty() && entry.prof != ProficiencyLevel::None)
            .map(move |entry| {
                let bonus = self.ability_modifier(entry.ability) + entry.prof.multiplier() * pb;
                (entry, bonus)
            })
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

    /// `(caster_level, caster_class_count)` for `pool`. Single class:
    /// PHB single-class table. Multi: PHB Multiclass Spellcaster Table.
    fn caster_info(&self, pool: SpellSlotPool) -> (u32, u32) {
        let entries: Vec<(u32, u32)> = self
            .identity
            .classes
            .iter()
            .filter_map(|cl| {
                let max_coef = self
                    .features
                    .iter()
                    .filter_map(|feature| {
                        if feature.source.as_class() != Some(cl.class.as_ref()) {
                            return None;
                        }
                        let spell_data = self.features.get(&feature.name)?.spells.as_ref()?;
                        (spell_data.pool == pool && spell_data.caster_coef != 0)
                            .then_some(spell_data.caster_coef)
                    })
                    .max()?;
                Some((cl.level, max_coef))
            })
            .collect();
        let caster_classes = entries.len() as u32;

        let caster_level = match entries.as_slice() {
            [] => 0,
            [(level, coef)] => SpellData::single_caster_level(*level, *coef),
            many => many.iter().map(|(level, coef)| level / coef).sum(),
        };

        (caster_level, caster_classes)
    }

    pub fn caster_level(&self, pool: SpellSlotPool) -> u32 {
        self.caster_info(pool).0
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
            && self.tools == other.tools
            && self.damage_modifiers == other.damage_modifiers
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

    /// True if there are forward-only changes that can be materialized
    /// without `rebuild()`: pending class levels (new class or new levels of
    /// an existing class), or species/background not yet applied while
    /// `features` is still empty (first-time application). Once any feature
    /// has been applied, species/background cannot be inserted ahead of it
    /// — that case routes through `needs_rebuild()` instead. Callers should
    /// check `needs_rebuild()` first; apply only makes sense when `applied`
    /// is a strict prefix of `identity`.
    pub fn has_pending_apply(&self) -> bool {
        let no_features = self.features.is_empty();
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
        self.rebuild_reasons_iter().collect()
    }

    /// Iterator equivalent of `rebuild_reasons` — `needs_rebuild` short-
    /// circuits on the first match without allocating the `Vec`.
    fn rebuild_reasons_iter(&self) -> impl Iterator<Item = RebuildReason> {
        let has_features = !self.features.is_empty();

        (!self.identity.species.is_empty() && !self.applied.species && has_features)
            .then_some(RebuildReason::SpeciesChanged)
            .into_iter()
            .chain(
                (!self.identity.background.is_empty() && !self.applied.background && has_features)
                    .then_some(RebuildReason::BackgroundChanged),
            )
            .chain(
                self.applied
                    .levels
                    .iter()
                    .filter(|(_, lvls)| !lvls.is_empty())
                    .filter(|&(class, _)| {
                        !self.identity.classes.iter().any(|cl| &cl.class == class)
                    })
                    .map(|(class, _)| RebuildReason::ClassRemoved(class.clone())),
            )
            .chain(self.identity.classes.iter().filter_map(|cl| {
                if !cl.class.is_empty()
                    && let Some(lvls) = self.applied.levels.get(&cl.class)
                    && let Some(&max) = lvls.iter().max()
                    && max > cl.level
                {
                    return Some(RebuildReason::LevelLowered {
                        class: cl.class.as_ref().into(),
                        applied: max,
                        current: cl.level,
                    });
                }

                None
            }))
            .chain(
                self.identity
                    .classes
                    .iter()
                    .filter(|cl| {
                        !cl.class.is_empty()
                            && cl.subclass.as_deref().is_some_and(|picked| {
                                self.features.iter().any(|feature| {
                                    matches!(
                                        &feature.source,
                                        FeatureSource::Subclass(class_name, sub, _)
                                            if class_name.as_ref() == cl.class.as_ref()
                                                && sub.as_ref() != picked
                                    )
                                })
                            })
                    })
                    .map(|cl| RebuildReason::SubclassChanged {
                        class: cl.class.as_ref().into(),
                    }),
            )
            .chain(
                self.features
                    .list
                    .iter()
                    .filter(|feature| feature.removed)
                    .map(|feature| RebuildReason::FeatureRemoved(feature.name.clone())),
            )
            .chain({
                let has_levels = self.applied.levels.values().any(|lvls| !lvls.is_empty());
                let has_class_markers = self.features.iter().any(|feature| {
                    matches!(
                        feature.category,
                        FeatureCategory::System(IdentitySlot::Class)
                    )
                });
                (has_levels && has_features && !has_class_markers)
                    .then_some(RebuildReason::LegacyMissingSystemMarkers)
            })
    }

    /// True if `applied` references slots that no longer match `identity`.
    /// See `rebuild_reasons` for the breakdown.
    pub fn needs_rebuild(&self) -> bool {
        self.rebuild_reasons_iter().next().is_some()
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

impl Character {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fresh character carrying only the given identity — every other field
    /// starts from `Default`. Used by rebuild's `build_clean` as the accretion
    /// target and by the rebuild args-modal as the cascade base.
    pub fn from_identity(identity: CharacterIdentity) -> Self {
        Self {
            core: CharacterCore {
                identity,
                ..CharacterCore::default()
            },
            ..Self::default()
        }
    }

    pub fn clear(&mut self) {
        let id = self.id;
        let mut identity = mem::take(&mut self.identity);
        // Drop legacy empty class entries — they're noise from a prior
        // default and confuse downstream logic that treats class entries as
        // canonical.
        identity
            .classes
            .retain(|class_level| !class_level.class.is_empty());
        let personality = mem::take(&mut self.personality);
        *self = Self {
            id,
            core: CharacterCore {
                identity,
                ..CharacterCore::default()
            },
            personality,
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
}

impl Default for Character {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            core: CharacterCore::default(),
            equipment: Equipment::default(),
            personality: Personality::default(),
            notes: Vec::new(),
            updated_at: now_epoch_secs(),
            shared: false,
            schema_version: 1,
        }
    }
}

impl expr::Context<Attribute, i32> for Character {
    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        self.core.assign(var, value)
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        self.core.resolve(var)
    }
}

impl expr::Context<Attribute, i32> for CharacterCore {
    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        match var {
            Attribute::Ability(ability) => {
                self.abilities.set(ability, value.max(1) as u32);
            }
            Attribute::MaxHp => {
                self.combat.hp_max = value as u32;
            }
            Attribute::Hp => {
                self.combat.hp_current = (value as u32).clamp(0, self.combat.hp_max);
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
                self.skills.set(skill, ProficiencyLevel::from(value));
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
            Attribute::ToolProficiency(name) => {
                self.tools.add(name).prof = ProficiencyLevel::from(value);
            }
            Attribute::ToolAbility(name) => {
                if let Ok(ability) = Ability::try_from(value.max(0) as u8) {
                    self.tools.add(name).ability = ability;
                }
            }
            Attribute::ToolCount => {
                self.tools.set_count(value.max(0) as usize);
            }
            Attribute::Species(name) => {
                if value != 0 {
                    self.identity.species = name.to_string();
                } else if self.identity.species == name {
                    self.identity.species.clear();
                }
            }
            Attribute::Background(name) => {
                if value != 0 {
                    self.identity.background = name.to_string();
                } else if self.identity.background == name {
                    self.identity.background.clear();
                }
            }
            Attribute::ClassLevel(AttrKey::Named(name)) => {
                let new_level = value.max(0) as u32;
                let existing = self
                    .identity
                    .classes
                    .iter()
                    .position(|class_level| &*class_level.class == name);
                match (existing, new_level) {
                    (Some(idx), 0) => {
                        self.identity.classes.remove(idx);
                    }
                    (Some(idx), level) => {
                        self.identity.classes[idx].level = level;
                    }
                    (None, 0) => {}
                    (None, level) => {
                        self.identity.classes.push(ClassLevel {
                            class: name.into(),
                            level,
                            ..ClassLevel::default()
                        });
                    }
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
            Attribute::CasterLevel(None) => Ok(self.caster_level(SpellSlotPool::default()) as i32),
            Attribute::CasterLevel(Some(pool)) => Ok(self.caster_level(pool) as i32),
            Attribute::Slot(pool, n) => {
                let pool = pool.unwrap_or_default();
                Ok(self.spell_slots.get_slot(pool, n as u32).total as i32)
            }
            Attribute::SlotUsed(pool, n) => {
                let pool = pool.unwrap_or_default();
                Ok(self.spell_slots.get_slot(pool, n as u32).used as i32)
            }
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
            Attribute::ToolProficiency(name) => Ok(self.tools.level(name).multiplier()),
            Attribute::ToolAbility(name) => {
                Ok(self.tools.ability(name).map_or(0, |ability| ability as i32))
            }
            Attribute::ToolCount => Ok(self.tools.len() as i32),
            Attribute::Species(name) => Ok((self.identity.species == name) as i32),
            Attribute::Background(name) => Ok((self.identity.background == name) as i32),
            Attribute::ClassLevel(AttrKey::Named(name)) => Ok(self
                .identity
                .classes
                .iter()
                .find(|cl| &*cl.class == name)
                .map(|cl| cl.level as i32)
                .unwrap_or(0)),
            Attribute::ClassCount => Ok(self
                .identity
                .classes
                .iter()
                .filter(|cl| !cl.class.is_empty())
                .count() as i32),
            Attribute::Subclass(name) => Ok(self
                .identity
                .classes
                .iter()
                .any(|cl| cl.subclass.as_deref() == Some(name))
                as i32),
            Attribute::FeatCategory(cat) => Ok(self.features.has_category(cat) as i32),
            a if a.is_advantage() => Ok(0),
            other => Err(expr::Error::unsupported_var(other)),
        }
    }
}

#[cfg(test)]
impl Character {
    pub fn test_character() -> Character {
        use std::collections::BTreeMap;

        use crate::model::{
            ClassLevel, FeatureCategory, FeatureData, FeatureSource, Spell, SpellData,
        };

        Character {
            id: Uuid::nil(),
            core: CharacterCore {
                identity: CharacterIdentity {
                    classes: vec![ClassLevel {
                        class: "Bard".into(),
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
                features: Features::from_parts(
                    vec![Feature {
                        name: "Bardic Inspiration".into(),
                        label: None,
                        description: "Use a bonus action...".to_string(),
                        applied: true,
                        category: FeatureCategory::Class,
                        source: FeatureSource::Class("Bard".into(), 1),
                        inputs: Vec::new(),
                        replaces: None,
                        removed: false,
                    }],
                    BTreeMap::from([(
                        "Spellcasting (Bard)".into(),
                        FeatureData {
                            fields: Vec::new(),
                            spells: Some(SpellData {
                                casting_ability: Ability::Charisma,
                                caster_coef: 1,
                                pool: SpellSlotPool::Arcane,
                                spells: vec![Spell {
                                    name: "Vicious Mockery".into(),
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
                proficiencies: VecSet::new(),
                languages: VecSet::new(),
                tools: Tools::default(),
                damage_modifiers: DamageModifiers::default(),
                spell_slots: SpellSlots::default(),
                applied: Applied {
                    species: true,
                    background: true,
                    levels: BTreeMap::new(),
                },
            },
            equipment: Equipment::default(),
            personality: Personality {
                name: "Share Test".into(),
                alignment: Alignment::ChaoticGood,
                ..Personality::default()
            },
            notes: Vec::new(),
            updated_at: 0,
            shared: false,
            schema_version: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::{
        expr::Context as _,
        model::{
            Armor, AttrKey, ClassLevel, Currency, Expr, Feature, FeatureCategory, FeatureData,
            FeatureField, FeatureSource, FeatureValue, FreeUses, Money, Spell, SpellData,
        },
        rules::apply::ApplyContext,
        vecset::VecSet,
    };

    /// Build a minimal character for testing (avoids Default which calls
    /// js_sys::Date)
    fn test_character() -> Character {
        Character {
            id: Uuid::nil(),
            core: CharacterCore {
                identity: CharacterIdentity {
                    classes: vec![ClassLevel {
                        class: "Fighter".into(),
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
                features: Features::default(),
                proficiencies: [
                    Proficiency::LightArmor,
                    Proficiency::MediumArmor,
                    Proficiency::HeavyArmor,
                    Proficiency::Shields,
                ]
                .into_iter()
                .collect(),
                languages: VecSet::new(),
                tools: Tools::default(),
                damage_modifiers: DamageModifiers::default(),
                spell_slots: SpellSlots::default(),
                applied: Applied::default(),
            },
            personality: Personality {
                name: "Test".into(),
                ..Personality::default()
            },
            equipment: Equipment::default(),
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
            name: feature_name.into(),
            source: FeatureSource::Class(class_name.into(), 1),
            applied: true,
            ..Default::default()
        });
        ch.features.insert(
            feature_name.into(),
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
            class: "Wizard".into(),
            level: 3,
            ..ClassLevel::default()
        });
        assert_eq!(ch.level(), 8);
    }

    #[wasm_bindgen_test]
    fn level_no_classes_returns_zero() {
        // `level()` is the raw sum of class levels; UI callers that need
        // a `>= 1` floor use `.max(1)` themselves. Without that floor,
        // `target_level = level() + 1` for a fresh character correctly
        // produces `User(1)` for the first class pick.
        let mut ch = test_character();
        ch.identity.classes.clear();
        assert_eq!(ch.level(), 0);
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
    fn caster_level_third_caster_below_threshold() {
        let mut ch = test_character();
        ch.identity.classes[0].level = 1;
        make_caster(&mut ch, "Fighter", "Spellcasting", 3, SpellSlotPool::Arcane);
        assert_eq!(ch.caster_level(SpellSlotPool::Arcane), 0);
        ch.identity.classes[0].level = 2;
        assert_eq!(ch.caster_level(SpellSlotPool::Arcane), 0);
        ch.identity.classes[0].level = 3;
        assert_eq!(ch.caster_level(SpellSlotPool::Arcane), 1);
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
            class: "Paladin".into(),
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
            class: "Warlock".into(),
            level: 3,
            ..ClassLevel::default()
        });
        make_caster(&mut ch, "Warlock", "Pact Magic", 1, SpellSlotPool::Pact);
        // Arcane pool only sees Fighter
        assert_eq!(ch.caster_level(SpellSlotPool::Arcane), 5);
        // Pact pool only sees Warlock
        assert_eq!(ch.caster_level(SpellSlotPool::Pact), 3);
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
        ch.identity.classes[0].subclass = Some("Champion".into());
        assert_eq!(ch.class_summary(), "Fighter (Champion) 5");
    }

    #[wasm_bindgen_test]
    fn class_summary_multiclass() {
        let mut ch = test_character();
        ch.identity.classes.push(ClassLevel {
            class: "Rogue".into(),
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
        let mut currency = Currency {
            gp: 10,
            sp: 5,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(50))); // 5 sp
        assert_eq!(
            currency,
            Currency {
                gp: 10,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_breaks_higher_coin() {
        // 10 gp 0 sp — spend 5 sp should exchange 1 gp → 10 sp, leaving 9 gp 5 sp
        let mut currency = Currency {
            gp: 10,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(50))); // 5 sp
        assert_eq!(
            currency,
            Currency {
                gp: 9,
                sp: 5,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_insufficient_returns_false() {
        let mut currency = Currency {
            gp: 1,
            ..Default::default()
        };
        assert!(!currency.spend(Money::from_gp(2)));
        // Currency unchanged
        assert_eq!(
            currency,
            Currency {
                gp: 1,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_exact_total() {
        let mut currency = Currency {
            gp: 1,
            sp: 5,
            cp: 3,
            ..Default::default()
        };
        let total = currency.as_money();
        assert!(currency.spend(total));
        assert_eq!(currency, Currency::default());
    }

    #[wasm_bindgen_test]
    fn currency_spend_cp_from_sp() {
        // 0 cp, 1 sp → spend 5 cp → break 1 sp, return 5 cp change
        let mut currency = Currency {
            sp: 1,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(5)));
        assert_eq!(
            currency,
            Currency {
                cp: 5,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_cp_exact() {
        // Spend CP when CP is available
        let mut currency = Currency {
            cp: 10,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(7)));
        assert_eq!(
            currency,
            Currency {
                cp: 3,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_sp_from_ep() {
        // 1 ep 0 sp → spend 3 sp (30 cp) → break 1 ep, return 2 sp change
        let mut currency = Currency {
            ep: 1,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(30))); // 3 sp
        assert_eq!(
            currency,
            Currency {
                sp: 2,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_ep_exact() {
        // 2 ep → spend 1 ep (50 cp) → 1 ep (exact match, no break needed)
        let mut currency = Currency {
            ep: 2,
            sp: 3,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(50))); // 1 ep
        assert_eq!(
            currency,
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
        let mut currency = Currency {
            gp: 1,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(7)));
        assert_eq!(
            currency,
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
        let mut currency = Currency {
            pp: 1,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(30))); // 3 sp
        assert_eq!(
            currency,
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
        let mut currency = Currency {
            gp: 2,
            sp: 3,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(150))); // 15 sp
        assert_eq!(
            currency,
            Currency {
                sp: 8,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_pp_exact() {
        // 2 pp → spend 1 pp (1000 cp) → 1 pp
        let mut currency = Currency {
            pp: 2,
            ..Default::default()
        };
        assert!(currency.spend(Money::from_cp(1000))); // 1 pp
        assert_eq!(
            currency,
            Currency {
                pp: 1,
                ..Default::default()
            }
        );
    }

    #[wasm_bindgen_test]
    fn currency_spend_zero() {
        // Spending 0 always succeeds and leaves currency unchanged
        let mut currency = Currency {
            gp: 5,
            sp: 3,
            ..Default::default()
        };
        assert!(currency.spend(Money::default()));
        assert_eq!(
            currency,
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
            ..Armor::default()
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
            class: "Wizard".into(),
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
            class: "Wizard".into(),
            level: 1,
            ..ClassLevel::default()
        });
        ch.applied.mark_level("Wizard", 1);
        ch.features.list.push(Feature {
            name: "Magic Missile".into(),
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
            class: "Wizard".into(),
            level: 1,
            ..ClassLevel::default()
        });
        ch.applied.mark_level("Wizard", 1);
        ch.features.list.push(Feature {
            name: "Magic Missile".into(),
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
            class: "Wizard".into(),
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
            class: "Cleric".into(),
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
            class: "Wizard".into(),
            level: 3,
            ..ClassLevel::default()
        });
        for lvl in 1..=5 {
            ch.applied.mark_level("Wizard", lvl);
        }
        assert!(ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn legacy_char_without_system_markers_needs_rebuild() {
        let mut ch = drift_character();
        ch.identity.classes.push(ClassLevel {
            class: "Fighter".into(),
            level: 3,
            ..ClassLevel::default()
        });
        for lvl in 1..=3 {
            ch.applied.mark_level("Fighter", lvl);
        }
        // Pre-marker character: only Class-source features in features.list.
        for lvl in 1..=3 {
            ch.features.list.push(Feature {
                name: format!("Fighter L{lvl} grant").into(),
                source: FeatureSource::Class("Fighter".into(), lvl),
                applied: true,
                category: FeatureCategory::Class,
                ..Feature::default()
            });
        }
        assert!(ch.needs_rebuild());
        assert!(
            ch.rebuild_reasons()
                .contains(&RebuildReason::LegacyMissingSystemMarkers)
        );
    }

    #[wasm_bindgen_test]
    fn drift_user_removed_feature_emits_feature_removed() {
        // User trashes a feature row. The soft-delete flag drives the drift
        // reason so the Rebuild banner shows.
        let mut ch = drift_character();
        ch.identity.species = "Human".to_string();
        ch.identity.background = "Soldier".to_string();
        ch.applied.species = true;
        ch.applied.background = true;
        ch.features.list.push(Feature {
            name: "Tough".into(),
            source: FeatureSource::User(0),
            applied: true,
            category: FeatureCategory::General,
            ..Feature::default()
        });

        ch.features.remove(0);

        let reasons = ch.rebuild_reasons();
        assert_eq!(reasons.len(), 1, "exactly one drift reason: {reasons:?}");
        assert_eq!(reasons[0], RebuildReason::FeatureRemoved("Tough".into()));
        assert!(ch.needs_rebuild());
    }

    #[wasm_bindgen_test]
    fn drift_readd_does_not_clear_feature_removed_until_rebuild() {
        // Re-adding the same name via raw push leaves the zombie alongside
        // the new live row; the banner persists until rebuild compacts.
        let mut ch = drift_character();
        ch.identity.species = "Human".to_string();
        ch.identity.background = "Soldier".to_string();
        ch.applied.species = true;
        ch.applied.background = true;
        ch.features.list.push(Feature {
            name: "Tough".into(),
            source: FeatureSource::User(0),
            applied: true,
            category: FeatureCategory::General,
            ..Feature::default()
        });
        ch.features.remove(0);

        // Simulate AddFeatureRow re-adding the same name.
        ch.features.list.push(Feature {
            name: "Tough".into(),
            source: FeatureSource::User(0),
            applied: true,
            category: FeatureCategory::General,
            ..Feature::default()
        });

        assert!(
            ch.rebuild_reasons().iter().any(
                |reason| matches!(reason, RebuildReason::FeatureRemoved(name) if &**name == "Tough")
            ),
            "tombstone still emits drift; derived state remains stale until rebuild",
        );
    }

    #[wasm_bindgen_test]
    fn drift_no_feature_removed_without_soft_deletes() {
        let mut ch = drift_character();
        ch.identity.species = "Human".to_string();
        ch.identity.background = "Soldier".to_string();
        ch.applied.species = true;
        ch.applied.background = true;
        ch.features.list.push(Feature {
            name: "Tough".into(),
            source: FeatureSource::User(0),
            applied: true,
            category: FeatureCategory::General,
            ..Feature::default()
        });

        assert!(
            !ch.rebuild_reasons()
                .iter()
                .any(|reason| matches!(reason, RebuildReason::FeatureRemoved(_))),
        );
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
            class: "Wizard".into(),
            level: 3,
            ..ClassLevel::default()
        });
        ch.identity.classes.push(ClassLevel {
            class: "Cleric".into(),
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

    // --- new spell-attribute resolvers (Context-level) ---

    fn caster_character() -> Character {
        let mut ch = test_character();
        ch.identity.classes[0] = ClassLevel {
            class: "Wizard".into(),
            level: 5,
            ..ClassLevel::default()
        };
        ch.features.put(
            "Spellcasting (Wizard)",
            None,
            String::new(),
            FeatureCategory::Class,
            FeatureSource::Class("Wizard".into(), 1),
            Vec::new(),
            true,
        );
        ch.features
            .entry("Spellcasting (Wizard)".into())
            .or_default()
            .spells
            .get_or_insert(SpellData {
                casting_ability: Ability::Intelligence,
                caster_coef: 1,
                pool: SpellSlotPool::Arcane,
                spells: Vec::new(),
                known: None,
            });
        ch
    }

    fn caster_ctx(ch: &mut Character) -> ApplyContext<'_> {
        let feature_pos = ch
            .features
            .list
            .iter()
            .position(|feature| &*feature.name == "Spellcasting (Wizard)")
            .expect("caster feature");
        ApplyContext::new(ch, feature_pos)
    }

    #[wasm_bindgen_test]
    fn slot_assign_overwrites_total() {
        let mut ch = caster_character();
        let mut ctx = caster_ctx(&mut ch);
        ctx.assign(Attribute::Slot(None, 1), 4).unwrap();
        ctx.assign(Attribute::Slot(None, 2), 3).unwrap();
        let slots = &ctx.character.spell_slots[&SpellSlotPool::Arcane];
        assert_eq!(slots[0].total, 4);
        assert_eq!(slots[1].total, 3);
    }

    #[wasm_bindgen_test]
    fn slot_assign_clamps_used_on_shrink() {
        let mut ch = caster_character();
        let levels = ch.spell_slots.entry(SpellSlotPool::Arcane).or_default();
        levels[0].total = 4;
        levels[0].used = 3;
        let mut ctx = caster_ctx(&mut ch);
        ctx.assign(Attribute::Slot(None, 1), 1).unwrap();
        let slot = &ctx.character.spell_slots[&SpellSlotPool::Arcane][0];
        assert_eq!(slot.total, 1);
        assert_eq!(slot.used, 1);
    }

    #[wasm_bindgen_test]
    fn slot_explicit_pool() {
        let mut ch = caster_character();
        let mut ctx = caster_ctx(&mut ch);
        ctx.assign(Attribute::Slot(Some(SpellSlotPool::Pact), 5), 2)
            .unwrap();
        assert_eq!(ctx.character.spell_slots[&SpellSlotPool::Pact][4].total, 2);
    }

    #[wasm_bindgen_test]
    fn slot_used_clamped_to_total() {
        let mut ch = caster_character();
        let mut ctx = caster_ctx(&mut ch);
        ctx.assign(Attribute::Slot(None, 1), 3).unwrap();
        ctx.assign(Attribute::SlotUsed(None, 1), 99).unwrap();
        let slot = &ctx.character.spell_slots[&SpellSlotPool::Arcane][0];
        assert_eq!(slot.total, 3);
        assert_eq!(slot.used, 3);
    }

    #[wasm_bindgen_test]
    fn caster_meta_assigns() {
        let mut ch = caster_character();
        let mut ctx = caster_ctx(&mut ch);
        ctx.assign(Attribute::SlotPool, 1).unwrap();
        ctx.assign(Attribute::CasterAbility, 5).unwrap(); // CHA
        ctx.assign(Attribute::CasterCoef, 2).unwrap();
        let data = ctx
            .character
            .features
            .spell_data("Spellcasting (Wizard)")
            .unwrap();
        assert_eq!(data.pool, SpellSlotPool::Pact);
        assert_eq!(data.casting_ability, Ability::Charisma);
        assert_eq!(data.caster_coef, 2);
    }

    #[wasm_bindgen_test]
    fn spell_cantrips_extend_and_trim() {
        let mut ch = caster_character();
        let mut ctx = caster_ctx(&mut ch);
        ctx.assign(Attribute::SpellCantrips, 3).unwrap();
        let data = ctx
            .character
            .features
            .spell_data("Spellcasting (Wizard)")
            .unwrap();
        assert_eq!(
            data.spells.iter().filter(|spell| spell.level == 0).count(),
            3
        );

        // Shrink to 1; empty placeholders removed first.
        let mut ctx = caster_ctx(&mut ch);
        ctx.assign(Attribute::SpellCantrips, 1).unwrap();
        let data = ctx
            .character
            .features
            .spell_data("Spellcasting (Wizard)")
            .unwrap();
        assert_eq!(
            data.spells.iter().filter(|spell| spell.level == 0).count(),
            1
        );
    }

    #[wasm_bindgen_test]
    fn spell_ready_pad_with_highest_slot_level() {
        let mut ch = caster_character();
        // Outfit with slot level 3.
        let mut ctx = caster_ctx(&mut ch);
        ctx.assign(Attribute::Slot(None, 1), 4).unwrap();
        ctx.assign(Attribute::Slot(None, 2), 3).unwrap();
        ctx.assign(Attribute::Slot(None, 3), 2).unwrap();
        ctx.assign(Attribute::SpellReady, 4).unwrap();
        let data = ctx
            .character
            .features
            .spell_data("Spellcasting (Wizard)")
            .unwrap();
        assert_eq!(
            data.spells.iter().filter(|spell| spell.level > 0).count(),
            4
        );
        for spell in data.spells.iter().filter(|spell| spell.level > 0) {
            assert_eq!(spell.level, 3);
        }
    }

    #[wasm_bindgen_test]
    fn spell_ready_preserves_sticky_on_shrink() {
        use crate::model::Spell;

        let mut ch = caster_character();
        ch.features
            .spell_data_mut("Spellcasting (Wizard)")
            .unwrap()
            .spells
            .push(Spell {
                name: "Fireball".into(),
                level: 3,
                sticky: true,
                ..Default::default()
            });
        let mut ctx = caster_ctx(&mut ch);
        ctx.assign(Attribute::SpellReady, 0).unwrap();
        let data = ctx
            .character
            .features
            .spell_data("Spellcasting (Wizard)")
            .unwrap();
        assert_eq!(data.spells.len(), 1);
        assert!(data.spells[0].sticky);
        assert_eq!(data.spells[0].name, "Fireball");
    }

    #[wasm_bindgen_test]
    fn slot_no_feature_errors() {
        // ApplyContext requires a real feature row; without spell data on
        // that feature, implicit-pool resolution must error.
        let mut ch = test_character();
        ch.features.list.push(Feature {
            name: "Bare".into(),
            applied: true,
            source: FeatureSource::User(0),
            ..Default::default()
        });
        let mut ctx = ApplyContext::new(&mut ch, 0);
        assert!(ctx.assign(Attribute::Slot(None, 1), 4).is_err());
    }

    #[wasm_bindgen_test]
    fn caster_level_dynamic_through_feature() {
        let mut ch = caster_character();
        let ctx = caster_ctx(&mut ch);
        // Wizard L5, full caster (coef=1) → CASTER.LEVEL.ARCANE = 5
        let level = ctx.resolve(Attribute::CasterLevel(None)).unwrap();
        assert_eq!(level, 5);
    }

    // --- Named-pool assign/resolve via Context (Stage I Task 4) ---

    /// Build a feature-scoped Context anchored on a single applied feature
    /// with no fields. Used for Named-pool tests so lazy-creation lands in
    /// a known feature data entry.
    fn named_pool_ctx_setup(feature_name: &str) -> Character {
        let mut ch = test_character();
        ch.features.list.push(Feature {
            name: feature_name.into(),
            applied: true,
            source: FeatureSource::User(0),
            ..Default::default()
        });
        ch.features
            .data_mut()
            .insert(feature_name.into(), FeatureData::default());
        ch
    }

    fn named_pool_ctx<'a>(ch: &'a mut Character, feature_name: &str) -> ApplyContext<'a> {
        let feature_pos = ch
            .features
            .list
            .iter()
            .position(|feature| &*feature.name == feature_name)
            .expect("feature in list");
        ApplyContext::new(ch, feature_pos)
    }

    #[wasm_bindgen_test]
    fn points_named_lazy_creates_field() {
        let mut ch = named_pool_ctx_setup("Rage");
        let mut ctx = named_pool_ctx(&mut ch, "Rage");
        ctx.assign(Attribute::PointsMax(AttrKey::Named("Charges")), 4)
            .unwrap();
        let data = ctx.character.features.get("Rage").unwrap();
        assert_eq!(data.fields.len(), 1);
        assert_eq!(data.fields[0].name, "Charges");
        match &data.fields[0].value {
            FeatureValue::Points { used, max } => {
                assert_eq!(*used, 0);
                assert_eq!(*max, 4);
            }
            other => panic!("expected Points, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn points_named_resolve_reads_back() {
        let mut ch = named_pool_ctx_setup("Rage");
        let mut ctx = named_pool_ctx(&mut ch, "Rage");
        ctx.assign(Attribute::PointsMax(AttrKey::Named("Charges")), 5)
            .unwrap();
        ctx.assign(Attribute::Points(AttrKey::Named("Charges")), 3)
            .unwrap();
        // Available = max(5) - used(2) = 3
        let available = ctx
            .resolve(Attribute::Points(AttrKey::Named("Charges")))
            .unwrap();
        assert_eq!(available, 3);
        let max = ctx
            .resolve(Attribute::PointsMax(AttrKey::Named("Charges")))
            .unwrap();
        assert_eq!(max, 5);
    }

    #[wasm_bindgen_test]
    fn points_named_subsequent_assign_updates_existing() {
        let mut ch = named_pool_ctx_setup("Rage");
        let mut ctx = named_pool_ctx(&mut ch, "Rage");
        ctx.assign(Attribute::PointsMax(AttrKey::Named("Charges")), 4)
            .unwrap();
        ctx.assign(Attribute::PointsMax(AttrKey::Named("Charges")), 6)
            .unwrap();
        let data = ctx.character.features.get("Rage").unwrap();
        // Single field, no duplicates.
        assert_eq!(data.fields.len(), 1);
        match &data.fields[0].value {
            FeatureValue::Points { max, .. } => assert_eq!(*max, 6),
            other => panic!("expected Points, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn die_named_lazy_creates_and_reads_back() {
        let mut ch = named_pool_ctx_setup("Bardic Inspiration");
        let mut ctx = named_pool_ctx(&mut ch, "Bardic Inspiration");
        ctx.assign(Attribute::DieSides(AttrKey::Named("Inspiration Die")), 8)
            .unwrap();
        ctx.assign(Attribute::DieCount(AttrKey::Named("Inspiration Die")), 3)
            .unwrap();
        let sides = ctx
            .resolve(Attribute::DieSides(AttrKey::Named("Inspiration Die")))
            .unwrap();
        let count = ctx
            .resolve(Attribute::DieCount(AttrKey::Named("Inspiration Die")))
            .unwrap();
        assert_eq!(sides, 8);
        assert_eq!(count, 3);
        // Single Die field present, not duplicated.
        let data = ctx.character.features.get("Bardic Inspiration").unwrap();
        assert_eq!(data.fields.len(), 1);
    }

    #[wasm_bindgen_test]
    fn bonus_named_lazy_creates_and_reads_back() {
        let mut ch = named_pool_ctx_setup("Custom Lineage");
        let mut ctx = named_pool_ctx(&mut ch, "Custom Lineage");
        ctx.assign(Attribute::Bonus(AttrKey::Named("AC Bonus")), 2)
            .unwrap();
        let value = ctx
            .resolve(Attribute::Bonus(AttrKey::Named("AC Bonus")))
            .unwrap();
        assert_eq!(value, 2);
        // Replace with new value — no duplicate.
        ctx.assign(Attribute::Bonus(AttrKey::Named("AC Bonus")), -1)
            .unwrap();
        let data = ctx.character.features.get("Custom Lineage").unwrap();
        assert_eq!(data.fields.len(), 1);
        match &data.fields[0].value {
            FeatureValue::Bonus(value) => assert_eq!(*value, -1),
            other => panic!("expected Bonus, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn choice_count_named_resizes_options() {
        let mut ch = named_pool_ctx_setup("Skill Picks");
        let mut ctx = named_pool_ctx(&mut ch, "Skill Picks");
        ctx.assign(Attribute::ChoiceCount(AttrKey::Named("Skills")), 3)
            .unwrap();
        let count = ctx
            .resolve(Attribute::ChoiceCount(AttrKey::Named("Skills")))
            .unwrap();
        assert_eq!(count, 3);
        let data = ctx.character.features.get("Skill Picks").unwrap();
        match &data.fields[0].value {
            FeatureValue::Choice { options } => assert_eq!(options.len(), 3),
            other => panic!("expected Choice, got {other:?}"),
        }
        // Shrink.
        ctx.assign(Attribute::ChoiceCount(AttrKey::Named("Skills")), 1)
            .unwrap();
        let data = ctx.character.features.get("Skill Picks").unwrap();
        match &data.fields[0].value {
            FeatureValue::Choice { options } => assert_eq!(options.len(), 1),
            other => panic!("expected Choice, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn cross_feature_named_lookup_writes_existing() {
        // Two features in pipeline order: A (scope), B (already has the field).
        let mut ch = test_character();
        ch.features.list.push(Feature {
            name: "Feature A".into(),
            applied: true,
            source: FeatureSource::User(0),
            ..Default::default()
        });
        ch.features.list.push(Feature {
            name: "Feature B".into(),
            applied: true,
            source: FeatureSource::User(1),
            ..Default::default()
        });
        ch.features
            .data_mut()
            .insert("Feature A".into(), FeatureData::default());
        ch.features.data_mut().insert(
            "Feature B".into(),
            FeatureData {
                fields: vec![FeatureField {
                    name: "Shared".into(),
                    label: None,
                    description: String::new(),
                    value: FeatureValue::Bonus(1),
                }],
                spells: None,
            },
        );
        let mut ctx = named_pool_ctx(&mut ch, "Feature A");
        ctx.assign(Attribute::Bonus(AttrKey::Named("Shared")), 7)
            .unwrap();
        // B's field mutated; A's data did not gain a new field.
        let data_a = ctx.character.features.get("Feature A").unwrap();
        let data_b = ctx.character.features.get("Feature B").unwrap();
        assert!(data_a.fields.is_empty());
        assert_eq!(data_b.fields.len(), 1);
        match &data_b.fields[0].value {
            FeatureValue::Bonus(value) => assert_eq!(*value, 7),
            other => panic!("expected Bonus, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn die_used_lazy_create_clamps_to_zero() {
        let mut ch = named_pool_ctx_setup("Bardic Inspiration");
        let mut ctx = named_pool_ctx(&mut ch, "Bardic Inspiration");
        // No die exists yet; DieUsed lazy-creates `Die{sides:0, amount:0}`
        // and clamps the assigned value to 0 (matches Points used-clamp on
        // out-of-order writes). Subsequent SIDES/COUNT writes will fill in
        // the rest of the structure without resetting `used`.
        ctx.assign(Attribute::DieUsed(AttrKey::Named("Inspiration Die")), 5)
            .unwrap();
        let entry = ctx.character.features.get("Bardic Inspiration").unwrap();
        assert_eq!(entry.fields.len(), 1);
        match &entry.fields[0].value {
            FeatureValue::Die { die, used } => {
                assert_eq!(die.sides, 0);
                assert_eq!(die.amount, 0);
                assert_eq!(*used, 0);
            }
            other => panic!("expected Die, got {other:?}"),
        }
    }

    #[wasm_bindgen_test]
    fn named_mismatched_value_type_errors() {
        let mut ch = named_pool_ctx_setup("Mixed");
        // Pre-create a Bonus field, then try Points-typed write to same name.
        ch.features
            .data_mut()
            .get_mut("Mixed")
            .unwrap()
            .fields
            .push(FeatureField {
                name: "X".into(),
                label: None,
                description: String::new(),
                value: FeatureValue::Bonus(0),
            });
        let mut ctx = named_pool_ctx(&mut ch, "Mixed");
        let outcome = ctx.assign(Attribute::PointsMax(AttrKey::Named("X")), 4);
        assert!(outcome.is_err());
        // Read of mismatched-type also errors.
        let read = ctx.resolve(Attribute::Points(AttrKey::Named("X")));
        assert!(read.is_err());
    }

    #[wasm_bindgen_test]
    fn die_used_and_die_sides_scoped_read_errors() {
        let mut ch = named_pool_ctx_setup("Anything");
        let ctx = named_pool_ctx(&mut ch, "Anything");
        // Scoped form is write-only-or-error for new variants.
        assert!(ctx.resolve(Attribute::DieSides(AttrKey::Scoped)).is_err());
        assert!(ctx.resolve(Attribute::DieCount(AttrKey::Scoped)).is_err());
        assert!(ctx.resolve(Attribute::DieUsed(AttrKey::Scoped)).is_err());
        assert!(ctx.resolve(Attribute::Bonus(AttrKey::Scoped)).is_err());
        assert!(
            ctx.resolve(Attribute::ChoiceCount(AttrKey::Scoped))
                .is_err()
        );
    }

    // --- STICKY / FREE_USES handlers (Stage II Task 6) ---

    #[wasm_bindgen_test]
    fn sticky_creates_spell_row() {
        let mut ch = named_pool_ctx_setup("Warlock Cantrips");
        let mut ctx = named_pool_ctx(&mut ch, "Warlock Cantrips");
        ctx.assign(Attribute::Sticky(AttrKey::named("Misty Step")), 1)
            .unwrap();
        let data = ctx
            .character
            .features
            .spell_data("Warlock Cantrips")
            .expect("SpellData lazy-created");
        assert_eq!(data.spells.len(), 1);
        let spell = &data.spells[0];
        assert_eq!(spell.name, "Misty Step");
        assert!(spell.sticky);
        assert_eq!(spell.level, 0);
        assert_eq!(spell.cost, 0);
        assert!(spell.free_uses.is_none());
        // Read: STICKY is 1 for present sticky row.
        let read = ctx
            .resolve(Attribute::Sticky(AttrKey::named("Misty Step")))
            .unwrap();
        assert_eq!(read, 1);
    }

    #[wasm_bindgen_test]
    fn sticky_idempotent_reapply() {
        let mut ch = named_pool_ctx_setup("Some Feature");
        let mut ctx = named_pool_ctx(&mut ch, "Some Feature");
        ctx.assign(Attribute::Sticky(AttrKey::named("Mage Hand")), 1)
            .unwrap();
        ctx.assign(Attribute::Sticky(AttrKey::named("Mage Hand")), 1)
            .unwrap();
        let data = ctx.character.features.spell_data("Some Feature").unwrap();
        assert_eq!(data.spells.len(), 1, "expected single sticky row");
    }

    #[wasm_bindgen_test]
    fn sticky_drop_only_sticky_rows() {
        let mut ch = named_pool_ctx_setup("Drop Test");
        // Pre-populate SpellData with one sticky + one user-prepared row, same
        // name, simulating the "user manually prepared a sticky-also spell"
        // edge case.
        let entry = ch.features.data_mut().get_mut("Drop Test").unwrap();
        let spell_data = entry.spells.get_or_insert_with(SpellData::default);
        spell_data.spells.push(Spell {
            name: "Bless".into(),
            sticky: true,
            ..Default::default()
        });
        spell_data.spells.push(Spell {
            name: "Bless".into(),
            sticky: false,
            level: 1,
            ..Default::default()
        });
        let mut ctx = named_pool_ctx(&mut ch, "Drop Test");
        ctx.assign(Attribute::Sticky(AttrKey::named("Bless")), 0)
            .unwrap();
        let data = ctx.character.features.spell_data("Drop Test").unwrap();
        assert_eq!(data.spells.len(), 1);
        assert!(!data.spells[0].sticky, "non-sticky row preserved");
        assert_eq!(data.spells[0].level, 1);
    }

    #[wasm_bindgen_test]
    fn free_uses_per_spell_set_max() {
        let mut ch = named_pool_ctx_setup("Pact Magic");
        let mut ctx = named_pool_ctx(&mut ch, "Pact Magic");
        ctx.assign(Attribute::Sticky(AttrKey::named("Misty Step")), 1)
            .unwrap();
        ctx.assign(Attribute::FreeUses(AttrKey::named("Misty Step")), 3)
            .unwrap();
        let data = ctx.character.features.spell_data("Pact Magic").unwrap();
        let spell = data
            .spells
            .iter()
            .find(|spell| spell.name == "Misty Step")
            .unwrap();
        let fu = spell.free_uses.as_ref().expect("free_uses populated");
        assert_eq!(fu.max, 3);
        assert_eq!(fu.used, 0);
        // Read back via Context.
        let max_read = ctx
            .resolve(Attribute::FreeUses(AttrKey::named("Misty Step")))
            .unwrap();
        assert_eq!(max_read, 3);
    }

    #[wasm_bindgen_test]
    fn free_uses_scoped_broadcast() {
        let mut ch = named_pool_ctx_setup("Channel Divinity");
        // Two cost>0 spells + one cost=0 spell. Scoped FREE_USES write
        // should touch only the cost-bearing rows.
        let entry = ch.features.data_mut().get_mut("Channel Divinity").unwrap();
        let spell_data = entry.spells.get_or_insert_with(SpellData::default);
        spell_data.spells.push(Spell {
            name: "Sacred Flame".into(),
            sticky: true,
            cost: 1,
            ..Default::default()
        });
        spell_data.spells.push(Spell {
            name: "Bless".into(),
            sticky: true,
            cost: 1,
            ..Default::default()
        });
        spell_data.spells.push(Spell {
            name: "Light".into(),
            sticky: true,
            cost: 0,
            ..Default::default()
        });
        let mut ctx = named_pool_ctx(&mut ch, "Channel Divinity");
        ctx.assign(Attribute::FreeUses(AttrKey::Scoped), 5).unwrap();
        let data = ctx
            .character
            .features
            .spell_data("Channel Divinity")
            .unwrap();
        let sacred = data
            .spells
            .iter()
            .find(|spell| spell.name == "Sacred Flame")
            .unwrap();
        let bless = data
            .spells
            .iter()
            .find(|spell| spell.name == "Bless")
            .unwrap();
        let light = data
            .spells
            .iter()
            .find(|spell| spell.name == "Light")
            .unwrap();
        assert_eq!(sacred.free_uses.as_ref().unwrap().max, 5);
        assert_eq!(bless.free_uses.as_ref().unwrap().max, 5);
        assert!(light.free_uses.is_none(), "cost=0 spells left untouched");
    }

    #[wasm_bindgen_test]
    fn free_uses_used_scoped_resets() {
        let mut ch = named_pool_ctx_setup("Reset Pool");
        let entry = ch.features.data_mut().get_mut("Reset Pool").unwrap();
        let spell_data = entry.spells.get_or_insert_with(SpellData::default);
        spell_data.spells.push(Spell {
            name: "A".into(),
            cost: 1,
            free_uses: Some(FreeUses { used: 2, max: 3 }),
            ..Default::default()
        });
        spell_data.spells.push(Spell {
            name: "B".into(),
            cost: 1,
            free_uses: Some(FreeUses { used: 1, max: 3 }),
            ..Default::default()
        });
        let mut ctx = named_pool_ctx(&mut ch, "Reset Pool");
        ctx.assign(Attribute::FreeUsesUsed(AttrKey::Scoped), 0)
            .unwrap();
        let data = ctx.character.features.spell_data("Reset Pool").unwrap();
        for spell in &data.spells {
            assert_eq!(spell.free_uses.as_ref().unwrap().used, 0);
        }
    }

    #[wasm_bindgen_test]
    fn sticky_scoped_write_errors() {
        let mut ch = named_pool_ctx_setup("Anything");
        let mut ctx = named_pool_ctx(&mut ch, "Anything");
        let outcome = ctx.assign(Attribute::Sticky(AttrKey::Scoped), 1);
        assert!(outcome.is_err(), "Scoped Sticky write must error");
        let read = ctx.resolve(Attribute::Sticky(AttrKey::Scoped));
        assert!(read.is_err(), "Scoped Sticky read must error");
    }

    #[wasm_bindgen_test]
    fn free_uses_named_without_spell_errors() {
        let mut ch = named_pool_ctx_setup("Empty Feature");
        let mut ctx = named_pool_ctx(&mut ch, "Empty Feature");
        // No matching spell row — must error (do not lazy-create).
        let outcome = ctx.assign(Attribute::FreeUses(AttrKey::named("Nonexistent")), 5);
        assert!(outcome.is_err());
        let used = ctx.assign(Attribute::FreeUsesUsed(AttrKey::named("Nonexistent")), 0);
        assert!(used.is_err());
    }

    #[wasm_bindgen_test]
    fn idempotent_sticky_doesnt_drop_user_prepared() {
        // Setup: one sticky row "Misty Step" + one user-prepared "Bless" of
        // different names. Re-evaluating STICKY = 1 for "Misty Step" must
        // leave both rows intact and not duplicate.
        let mut ch = named_pool_ctx_setup("OnCompute Reeval");
        let mut ctx = named_pool_ctx(&mut ch, "OnCompute Reeval");
        ctx.assign(Attribute::Sticky(AttrKey::named("Misty Step")), 1)
            .unwrap();
        // Manually add a user-prepared spell of a different name.
        let data = ctx
            .character
            .features
            .spell_data_mut("OnCompute Reeval")
            .unwrap();
        data.spells.push(Spell {
            name: "Bless".into(),
            sticky: false,
            level: 1,
            ..Default::default()
        });
        // Re-evaluate (OnCompute pass).
        ctx.assign(Attribute::Sticky(AttrKey::named("Misty Step")), 1)
            .unwrap();
        let data = ctx
            .character
            .features
            .spell_data("OnCompute Reeval")
            .unwrap();
        assert_eq!(data.spells.len(), 2);
        let misty = data
            .spells
            .iter()
            .find(|spell| spell.name == "Misty Step")
            .unwrap();
        let bless = data
            .spells
            .iter()
            .find(|spell| spell.name == "Bless")
            .unwrap();
        assert!(misty.sticky);
        assert!(!bless.sticky);
    }
}
