use std::{cell::RefCell, collections::HashSet, fmt, str::FromStr};

use leptos_fluent::{I18n, tr};
use serde::{Deserialize, Serialize};

use crate::model::{
    Ability, DamageType, FeatureCategory, Proficiency, Skill, SpellSlotPool, Translatable,
};

/// Identifies a named or scope-implicit pool/die/bonus/choice.
///
/// `Scoped` is the bare form (`POINTS`, `FREE_USES`) — resolves through the
/// current feature scope. `Named("X")` is the backtick-quoted explicit form
/// (`POINTS.\`X\``, `FREE_USES.\`X\``) — addresses a specific entry by name.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttrKey {
    Scoped,
    Named(&'static str),
}

impl AttrKey {
    pub fn named(name: &str) -> Self {
        Self::Named(intern(name))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Attribute {
    Ability(Ability),
    Modifier(Ability),
    SavingThrow(Ability),
    Skill(Skill),
    SkillProficiency(Skill),
    SaveProficiency(Ability),
    EquipmentProficiency(Proficiency),
    MaxHp,
    Hp,
    TempHp,
    Level,
    Ac,
    Speed,
    /// Class level: `Scoped` reads via `ApplyContext` (current class scope);
    /// `Named(name)` reads/writes the named class's level via
    /// `Character::assign`/`resolve`.
    ClassLevel(AttrKey),
    /// Number of distinct classes the character holds — read-only count of
    /// `identity.classes`. Used for first-class exemption in synthesized
    /// `System(Class)` prereqs (`<existing> or CLASS.COUNT == 0`).
    ClassCount,
    /// Hit dice available for the class scope (= max - used).
    HitDice,
    /// Total hit dice of the scoped class (= class level).
    HitDiceMax,
    /// Hit dice already spent for the scoped class.
    HitDiceUsed,
    /// Hit-die size for the scoped class (d6/d8/d10/d12).
    HitDiceSides,
    CasterLevel(Option<SpellSlotPool>),
    CasterModifier,
    ProfBonus,
    AttackBonus,
    Initiative,
    InitiativeBonus,
    Inspiration,
    AbilityAdvantage(Ability),
    SkillAdvantage(Skill),
    SaveAdvantage(Ability),
    AttackAdvantage,
    SpellDc,
    SpellAttack,
    SpellAttackAdvantage,
    SlotLevel,
    Points(AttrKey),
    PointsMax(AttrKey),
    DieSides(AttrKey),
    DieCount(AttrKey),
    DieUsed(AttrKey),
    Bonus(AttrKey),
    ChoiceCount(AttrKey),
    Sticky(AttrKey),
    FreeUses(AttrKey),
    FreeUsesUsed(AttrKey),
    Cost,
    /// Gear-local: current charges remaining (= max - used).
    Charges,
    /// Gear-local: maximum charges of the owning gear.
    ChargesMax,
    /// Gear-local: charges already spent.
    ChargesUsed,
    /// Gear-local: read-only count of remaining items in the stack.
    /// Decrement is structural via `ChoiceOption.consumes`.
    Quantity,
    Resistance(DamageType),
    Vulnerability(DamageType),
    Immunity(DamageType),
    DamageReduction(DamageType),
    Attacks,
    Slot(Option<SpellSlotPool>, u8),
    SlotUsed(Option<SpellSlotPool>, u8),
    SlotPool,
    CasterAbility,
    CasterCoef,
    SpellCantrips,
    SpellKnown,
    SpellReady,
    Arg(u8),
    Feature(&'static str),
    FeatCategory(FeatureCategory),
    Language(&'static str),
    Species(&'static str),
    Background(&'static str),
    Subclass(&'static str),
}

thread_local! {
    static INTERNED: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
}

/// Intern a string for the lifetime of the program.
/// Intentional leak — used for Feature attribute names that live
/// until the wasm instance (browser tab) is closed. Deduplicates
/// via a global HashSet so each unique name is leaked at most once.
pub fn intern(s: &str) -> &'static str {
    INTERNED.with_borrow_mut(|set| {
        if let Some(&existing) = set.get(s) {
            return existing;
        }
        let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
        set.insert(leaked);
        leaked
    })
}

/// Like `intern` but consumes an existing `Box<str>` — leaks the box
/// directly on dedupe miss (no extra copy / realloc). Drops the box when
/// the contents are already interned.
pub fn intern_box(s: Box<str>) -> &'static str {
    INTERNED.with_borrow_mut(|set| {
        if let Some(&existing) = set.get(s.as_ref()) {
            return existing;
        }
        let leaked: &'static str = Box::leak(s);
        set.insert(leaked);
        leaked
    })
}

impl Attribute {
    /// If this is an `Arg(n)`, returns `Some(n)`.
    pub fn arg_index(&self) -> Option<u8> {
        if let Self::Arg(n) = self {
            Some(*n)
        } else {
            None
        }
    }

    /// Returns true if this attribute represents an advantage/disadvantage
    /// flag.
    pub fn is_advantage(&self) -> bool {
        matches!(
            self,
            Self::AbilityAdvantage(_)
                | Self::SkillAdvantage(_)
                | Self::SaveAdvantage(_)
                | Self::AttackAdvantage
                | Self::SpellAttackAdvantage
        )
    }

    /// Returns true if this attribute is scoped (stored per-feature
    /// rather than globally when inside a scoped effect).
    pub fn is_scoped(&self) -> bool {
        matches!(
            self,
            Self::SpellDc
                | Self::SpellAttack
                | Self::SpellAttackAdvantage
                | Self::SlotPool
                | Self::CasterAbility
                | Self::CasterCoef
                | Self::SpellCantrips
                | Self::SpellKnown
                | Self::SpellReady
                | Self::HitDice
                | Self::HitDiceMax
                | Self::HitDiceUsed
                | Self::HitDiceSides
                | Self::Subclass(_)
        )
    }
}

/// Split a leading name segment from `input`. Returns `(name, remaining)`.
/// Backticks are optional — when the name has no special chars (`.`, backtick),
/// callers can omit them: `parse_backtick_name("Lucky.MAX")` →
/// `Ok(("Lucky", ".MAX"))`. With backticks: ``parse_backtick_name("`Sorcery
/// Points`.MAX")`` → `Ok(("Sorcery Points", ".MAX"))`.
fn parse_backtick_name(input: &str) -> Result<(&str, &str), &'static str> {
    if let Some(after_open) = input.strip_prefix('`') {
        let close_at = after_open.find('`').ok_or("unterminated backtick name")?;
        let name = &after_open[..close_at];
        if name.is_empty() {
            return Err("empty backtick name");
        }
        return Ok((name, &after_open[close_at + 1..]));
    }
    let end = input.find('.').unwrap_or(input.len());
    let name = &input[..end];
    if name.is_empty() {
        return Err("empty name");
    }
    Ok((name, &input[end..]))
}

/// Write an `AttrKey` rendering: `<PREFIX>` for Scoped,
/// `<PREFIX>.\`<name>\`<SUFFIX>` for Named. `suffix` is `""` for plain forms or
/// e.g. `".MAX"` / `".SIDES"`.
fn fmt_attr_key(
    key: &AttrKey,
    prefix: &str,
    suffix: &str,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match key {
        AttrKey::Scoped => {
            f.write_str(prefix)?;
            f.write_str(suffix)
        }
        AttrKey::Named(name) => write!(f, "{prefix}.`{name}`{suffix}"),
    }
}

pub fn parse_ability(s: &str) -> Option<Ability> {
    match s {
        "STR" => Some(Ability::Strength),
        "DEX" => Some(Ability::Dexterity),
        "CON" => Some(Ability::Constitution),
        "INT" => Some(Ability::Intelligence),
        "WIS" => Some(Ability::Wisdom),
        "CHA" => Some(Ability::Charisma),
        _ => None,
    }
}

fn parse_proficiency(s: &str) -> Option<Proficiency> {
    match s {
        "LIGHT_ARMOR" => Some(Proficiency::LightArmor),
        "MEDIUM_ARMOR" => Some(Proficiency::MediumArmor),
        "HEAVY_ARMOR" => Some(Proficiency::HeavyArmor),
        "SHIELDS" => Some(Proficiency::Shields),
        "SIMPLE_WEAPONS" => Some(Proficiency::SimpleWeapons),
        "MARTIAL_WEAPONS" => Some(Proficiency::MartialWeapons),
        _ => None,
    }
}

pub fn parse_damage_type(s: &str) -> Option<DamageType> {
    match s {
        "ACID" => Some(DamageType::Acid),
        "BLUDG" => Some(DamageType::Bludgeoning),
        "COLD" => Some(DamageType::Cold),
        "FIRE" => Some(DamageType::Fire),
        "FORCE" => Some(DamageType::Force),
        "LIGHT" => Some(DamageType::Lightning),
        "NECRO" => Some(DamageType::Necrotic),
        "PIERC" => Some(DamageType::Piercing),
        "POISON" => Some(DamageType::Poison),
        "PSYCH" => Some(DamageType::Psychic),
        "RADI" => Some(DamageType::Radiant),
        "SLASH" => Some(DamageType::Slashing),
        "THUND" => Some(DamageType::Thunder),
        _ => None,
    }
}

impl DamageType {
    fn abbr(self) -> &'static str {
        match self {
            Self::Acid => "ACID",
            Self::Bludgeoning => "BLUDG",
            Self::Cold => "COLD",
            Self::Fire => "FIRE",
            Self::Force => "FORCE",
            Self::Lightning => "LIGHT",
            Self::Necrotic => "NECRO",
            Self::Piercing => "PIERC",
            Self::Poison => "POISON",
            Self::Psychic => "PSYCH",
            Self::Radiant => "RADI",
            Self::Slashing => "SLASH",
            Self::Thunder => "THUND",
        }
    }
}

impl Proficiency {
    fn abbr(self) -> &'static str {
        match self {
            Self::LightArmor => "LIGHT_ARMOR",
            Self::MediumArmor => "MEDIUM_ARMOR",
            Self::HeavyArmor => "HEAVY_ARMOR",
            Self::Shields => "SHIELDS",
            Self::SimpleWeapons => "SIMPLE_WEAPONS",
            Self::MartialWeapons => "MARTIAL_WEAPONS",
        }
    }
}

pub fn parse_skill(s: &str) -> Option<Skill> {
    match s {
        "ACRO" => Some(Skill::Acrobatics),
        "ANIM" => Some(Skill::AnimalHandling),
        "ARCA" => Some(Skill::Arcana),
        "ATHL" => Some(Skill::Athletics),
        "DECE" => Some(Skill::Deception),
        "HIST" => Some(Skill::History),
        "INSI" => Some(Skill::Insight),
        "INTI" => Some(Skill::Intimidation),
        "INVE" => Some(Skill::Investigation),
        "MEDI" => Some(Skill::Medicine),
        "NATU" => Some(Skill::Nature),
        "PERC" => Some(Skill::Perception),
        "PERF" => Some(Skill::Performance),
        "PERS" => Some(Skill::Persuasion),
        "RELI" => Some(Skill::Religion),
        "SLEI" => Some(Skill::SleightOfHand),
        "STEA" => Some(Skill::Stealth),
        "SURV" => Some(Skill::Survival),
        _ => None,
    }
}

impl Skill {
    fn abbr(self) -> &'static str {
        match self {
            Self::Acrobatics => "ACRO",
            Self::AnimalHandling => "ANIM",
            Self::Arcana => "ARCA",
            Self::Athletics => "ATHL",
            Self::Deception => "DECE",
            Self::History => "HIST",
            Self::Insight => "INSI",
            Self::Intimidation => "INTI",
            Self::Investigation => "INVE",
            Self::Medicine => "MEDI",
            Self::Nature => "NATU",
            Self::Perception => "PERC",
            Self::Performance => "PERF",
            Self::Persuasion => "PERS",
            Self::Religion => "RELI",
            Self::SleightOfHand => "SLEI",
            Self::Stealth => "STEA",
            Self::Survival => "SURV",
        }
    }
}

/// Parse the suffix of `SLOT.…` attributes:
/// - `POOL` → `SlotPool`
/// - `N` / `ARCANE.N` / `PACT.N` → `Slot(pool, N)`
/// - `N.USED` / `ARCANE.N.USED` / `PACT.N.USED` → `SlotUsed(pool, N)`
///
/// `N` must be in `1..=9`.
fn parse_slot_suffix(rest: &str) -> Result<Attribute, &'static str> {
    if rest == "POOL" {
        return Ok(Attribute::SlotPool);
    }
    let (pool, rest) = if let Some(r) = rest.strip_prefix("ARCANE.") {
        (Some(SpellSlotPool::Arcane), r)
    } else if let Some(r) = rest.strip_prefix("PACT.") {
        (Some(SpellSlotPool::Pact), r)
    } else {
        (None, rest)
    };
    let (n_str, used) = if let Some(n) = rest.strip_suffix(".USED") {
        (n, true)
    } else {
        (rest, false)
    };
    let n: u8 = n_str.parse().map_err(|_| "invalid SLOT index")?;
    if !(1..=9).contains(&n) {
        return Err("SLOT index must be 1..=9");
    }
    Ok(if used {
        Attribute::SlotUsed(pool, n)
    } else {
        Attribute::Slot(pool, n)
    })
}

impl FromStr for Attribute {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Dotted forms: STR.MOD, STR.SAVE, STR.ADV, STR.SAVE.ADV,
        // SKILL.ACRO, SKILL.ACRO.ADV, ATK.ADV
        let Some((prefix, rest)) = s.split_once('.') else {
            return match s {
                "HP" => Ok(Self::Hp),
                "LEVEL" => Ok(Self::Level),
                "AC" => Ok(Self::Ac),
                "SPEED" => Ok(Self::Speed),
                "ATK" => Ok(Self::AttackBonus),
                "INIT" => Ok(Self::Initiative),
                "INSPIRATION" => Ok(Self::Inspiration),
                "ATTACKS" => Ok(Self::Attacks),
                "POINTS" => Ok(Self::Points(AttrKey::Scoped)),
                "FREE_USES" => Ok(Self::FreeUses(AttrKey::Scoped)),
                "COST" => Ok(Self::Cost),
                "CHARGES" => Ok(Self::Charges),
                "QUANTITY" => Ok(Self::Quantity),
                "HIT_DICE" => Ok(Self::HitDice),
                other => {
                    // Bare ability names => ability score
                    parse_ability(other)
                        .map(Self::Ability)
                        .ok_or("unknown attribute")
                }
            };
        };

        match prefix {
            "SKILL" => {
                // SKILL.ACRO or SKILL.ACRO.ADV
                let Some((skill_str, suffix)) = rest.split_once('.') else {
                    return parse_skill(rest).map(Self::Skill).ok_or("unknown skill");
                };
                match suffix {
                    "ADV" => parse_skill(skill_str)
                        .map(Self::SkillAdvantage)
                        .ok_or("unknown skill"),
                    "PROF" => parse_skill(skill_str)
                        .map(Self::SkillProficiency)
                        .ok_or("unknown skill"),
                    _ => Err("unknown skill suffix (expected ADV or PROF)"),
                }
            }
            "PROF" => match rest {
                "BONUS" => Ok(Self::ProfBonus),
                _ => parse_proficiency(rest)
                    .map(Self::EquipmentProficiency)
                    .ok_or("unknown proficiency"),
            },
            "HP" => match rest {
                "MAX" => Ok(Self::MaxHp),
                "TEMP" => Ok(Self::TempHp),
                _ => Err("unknown HP suffix (expected MAX or TEMP)"),
            },
            "CLASS" => match rest {
                "LEVEL" => Ok(Self::ClassLevel(AttrKey::Scoped)),
                "COUNT" => Ok(Self::ClassCount),
                _ => {
                    let (name, remaining) = parse_backtick_name(rest)?;
                    match remaining {
                        ".LEVEL" => Ok(Self::ClassLevel(AttrKey::named(name))),
                        _ => Err("unknown CLASS suffix (expected LEVEL or COUNT)"),
                    }
                }
            },
            "CASTER" => match rest {
                "LEVEL" => Ok(Self::CasterLevel(None)),
                "LEVEL.ARCANE" => Ok(Self::CasterLevel(Some(SpellSlotPool::Arcane))),
                "LEVEL.PACT" => Ok(Self::CasterLevel(Some(SpellSlotPool::Pact))),
                "MOD" => Ok(Self::CasterModifier),
                "ABILITY" => Ok(Self::CasterAbility),
                "COEF" => Ok(Self::CasterCoef),
                _ => Err("unknown CASTER suffix"),
            },
            "RESIST" => parse_damage_type(rest)
                .map(Self::Resistance)
                .ok_or("unknown damage type"),
            "VULN" => parse_damage_type(rest)
                .map(Self::Vulnerability)
                .ok_or("unknown damage type"),
            "IMMUNE" => parse_damage_type(rest)
                .map(Self::Immunity)
                .ok_or("unknown damage type"),
            "DR" => parse_damage_type(rest)
                .map(Self::DamageReduction)
                .ok_or("unknown damage type"),
            "HIT_DICE" => match rest {
                "MAX" => Ok(Self::HitDiceMax),
                "USED" => Ok(Self::HitDiceUsed),
                "SIDES" => Ok(Self::HitDiceSides),
                _ => Err("unknown HIT_DICE suffix (expected MAX, USED or SIDES)"),
            },
            "CHARGES" => match rest {
                "MAX" => Ok(Self::ChargesMax),
                "USED" => Ok(Self::ChargesUsed),
                _ => Err("unknown CHARGES suffix (expected MAX or USED)"),
            },
            "POINTS" => match rest {
                "MAX" => Ok(Self::PointsMax(AttrKey::Scoped)),
                _ => {
                    let (name, remaining) = parse_backtick_name(rest)?;
                    let key = AttrKey::named(name);
                    match remaining {
                        "" => Ok(Self::Points(key)),
                        ".MAX" => Ok(Self::PointsMax(key)),
                        _ => Err("unknown POINTS suffix"),
                    }
                }
            },
            "DIE" => {
                let (name, remaining) = parse_backtick_name(rest)?;
                let key = AttrKey::named(name);
                match remaining {
                    ".SIDES" => Ok(Self::DieSides(key)),
                    ".COUNT" => Ok(Self::DieCount(key)),
                    ".USED" => Ok(Self::DieUsed(key)),
                    _ => Err("unknown DIE suffix (expected SIDES/COUNT/USED)"),
                }
            }
            "BONUS" => {
                let (name, remaining) = parse_backtick_name(rest)?;
                if !remaining.is_empty() {
                    return Err("unknown BONUS suffix");
                }
                Ok(Self::Bonus(AttrKey::named(name)))
            }
            "CHOICE" => {
                let (name, remaining) = parse_backtick_name(rest)?;
                let key = AttrKey::named(name);
                match remaining {
                    ".COUNT" => Ok(Self::ChoiceCount(key)),
                    _ => Err("unknown CHOICE suffix (expected COUNT)"),
                }
            }
            "STICKY" => {
                let (name, remaining) = parse_backtick_name(rest)?;
                if !remaining.is_empty() {
                    return Err("unknown STICKY suffix");
                }
                Ok(Self::Sticky(AttrKey::named(name)))
            }
            "FREE_USES" => match rest {
                "USED" => Ok(Self::FreeUsesUsed(AttrKey::Scoped)),
                _ => {
                    let (name, remaining) = parse_backtick_name(rest)?;
                    let key = AttrKey::named(name);
                    match remaining {
                        "" => Ok(Self::FreeUses(key)),
                        ".USED" => Ok(Self::FreeUsesUsed(key)),
                        _ => Err("unknown FREE_USES suffix"),
                    }
                }
            },
            "INIT" => match rest {
                "BONUS" => Ok(Self::InitiativeBonus),
                _ => Err("unknown INIT suffix (expected BONUS)"),
            },
            "ARG" => rest
                .parse::<u8>()
                .map(Self::Arg)
                .map_err(|_| "invalid ARG index (expected integer 0-255)"),
            "ATK" => match rest {
                "ADV" => Ok(Self::AttackAdvantage),
                _ => Err("unknown ATK suffix (expected ADV)"),
            },
            "SPELL" => match rest {
                "DC" => Ok(Self::SpellDc),
                "ATK" => Ok(Self::SpellAttack),
                "ATK.ADV" => Ok(Self::SpellAttackAdvantage),
                "CANTRIPS" => Ok(Self::SpellCantrips),
                "KNOWN" => Ok(Self::SpellKnown),
                "READY" => Ok(Self::SpellReady),
                _ => Err("unknown SPELL suffix"),
            },
            "SLOT" => match rest {
                "LEVEL" => Ok(Self::SlotLevel),
                _ => parse_slot_suffix(rest),
            },
            "FEAT" => {
                let name = rest.trim_matches('`');
                Ok(Self::Feature(intern(name)))
            }
            "LANG" => {
                let name = rest.trim_matches('`');
                Ok(Self::Language(intern(name)))
            }
            "SPECIES" => {
                let name = rest.trim_matches('`');
                Ok(Self::Species(intern(name)))
            }
            "BACKGROUND" => {
                let name = rest.trim_matches('`');
                Ok(Self::Background(intern(name)))
            }
            "SUBCLASS" => {
                let name = rest.trim_matches('`');
                Ok(Self::Subclass(intern(name)))
            }
            "FEAT_CAT" => rest
                .parse::<FeatureCategory>()
                .map(Self::FeatCategory)
                .map_err(|_| "unknown feature category"),
            prefix => {
                let Some(ability) = parse_ability(prefix) else {
                    return Err("unknown attribute");
                };

                // STR.MOD, STR.SAVE, STR.ADV, STR.SAVE.ADV
                let Some((middle, suffix)) = rest.split_once('.') else {
                    return match rest {
                        "MOD" => Ok(Self::Modifier(ability)),
                        "SAVE" => Ok(Self::SavingThrow(ability)),
                        "ADV" => Ok(Self::AbilityAdvantage(ability)),
                        _ => Err("unknown ability suffix (expected MOD, SAVE, or ADV)"),
                    };
                };
                if middle != "SAVE" {
                    return Err("unknown ability suffix");
                };
                match suffix {
                    "ADV" => Ok(Self::SaveAdvantage(ability)),
                    "PROF" => Ok(Self::SaveProficiency(ability)),
                    _ => Err("unknown SAVE suffix (expected ADV or PROF)"),
                }
            }
        }
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Ability(ability) => write!(f, "{}", ability.abbr()),
            Self::Modifier(ability) => write!(f, "{}.MOD", ability.abbr()),
            Self::SavingThrow(ability) => write!(f, "{}.SAVE", ability.abbr()),
            Self::Skill(skill) => write!(f, "SKILL.{}", skill.abbr()),
            Self::SkillProficiency(skill) => write!(f, "SKILL.{}.PROF", skill.abbr()),
            Self::SaveProficiency(ability) => write!(f, "{}.SAVE.PROF", ability.abbr()),
            Self::EquipmentProficiency(prof) => write!(f, "PROF.{}", prof.abbr()),
            Self::MaxHp => f.write_str("HP.MAX"),
            Self::Hp => f.write_str("HP"),
            Self::TempHp => f.write_str("HP.TEMP"),
            Self::Level => f.write_str("LEVEL"),
            Self::Ac => f.write_str("AC"),
            Self::Speed => f.write_str("SPEED"),
            Self::ClassLevel(key) => fmt_attr_key(key, "CLASS", ".LEVEL", f),
            Self::ClassCount => f.write_str("CLASS.COUNT"),
            Self::HitDice => f.write_str("HIT_DICE"),
            Self::HitDiceMax => f.write_str("HIT_DICE.MAX"),
            Self::HitDiceUsed => f.write_str("HIT_DICE.USED"),
            Self::HitDiceSides => f.write_str("HIT_DICE.SIDES"),
            Self::CasterLevel(None) => f.write_str("CASTER.LEVEL"),
            Self::CasterLevel(Some(SpellSlotPool::Arcane)) => f.write_str("CASTER.LEVEL.ARCANE"),
            Self::CasterLevel(Some(SpellSlotPool::Pact)) => f.write_str("CASTER.LEVEL.PACT"),
            Self::CasterModifier => f.write_str("CASTER.MOD"),
            Self::ProfBonus => f.write_str("PROF.BONUS"),
            Self::AttackBonus => f.write_str("ATK"),
            Self::Initiative => f.write_str("INIT"),
            Self::InitiativeBonus => f.write_str("INIT.BONUS"),
            Self::Inspiration => f.write_str("INSPIRATION"),
            Self::AbilityAdvantage(ability) => write!(f, "{}.ADV", ability.abbr()),
            Self::SkillAdvantage(skill) => write!(f, "SKILL.{}.ADV", skill.abbr()),
            Self::SaveAdvantage(ability) => write!(f, "{}.SAVE.ADV", ability.abbr()),
            Self::AttackAdvantage => f.write_str("ATK.ADV"),
            Self::SpellDc => f.write_str("SPELL.DC"),
            Self::SpellAttack => f.write_str("SPELL.ATK"),
            Self::SpellAttackAdvantage => f.write_str("SPELL.ATK.ADV"),
            Self::SlotLevel => f.write_str("SLOT.LEVEL"),
            Self::Points(key) => fmt_attr_key(key, "POINTS", "", f),
            Self::PointsMax(key) => fmt_attr_key(key, "POINTS", ".MAX", f),
            Self::DieSides(key) => fmt_attr_key(key, "DIE", ".SIDES", f),
            Self::DieCount(key) => fmt_attr_key(key, "DIE", ".COUNT", f),
            Self::DieUsed(key) => fmt_attr_key(key, "DIE", ".USED", f),
            Self::Bonus(key) => fmt_attr_key(key, "BONUS", "", f),
            Self::ChoiceCount(key) => fmt_attr_key(key, "CHOICE", ".COUNT", f),
            Self::Sticky(key) => fmt_attr_key(key, "STICKY", "", f),
            Self::FreeUses(key) => fmt_attr_key(key, "FREE_USES", "", f),
            Self::FreeUsesUsed(key) => fmt_attr_key(key, "FREE_USES", ".USED", f),
            Self::Cost => f.write_str("COST"),
            Self::Charges => f.write_str("CHARGES"),
            Self::ChargesMax => f.write_str("CHARGES.MAX"),
            Self::ChargesUsed => f.write_str("CHARGES.USED"),
            Self::Quantity => f.write_str("QUANTITY"),
            Self::Resistance(dt) => write!(f, "RESIST.{}", dt.abbr()),
            Self::Vulnerability(dt) => write!(f, "VULN.{}", dt.abbr()),
            Self::Immunity(dt) => write!(f, "IMMUNE.{}", dt.abbr()),
            Self::DamageReduction(dt) => write!(f, "DR.{}", dt.abbr()),
            Self::Attacks => f.write_str("ATTACKS"),
            Self::Slot(None, n) => write!(f, "SLOT.{n}"),
            Self::Slot(Some(SpellSlotPool::Arcane), n) => write!(f, "SLOT.ARCANE.{n}"),
            Self::Slot(Some(SpellSlotPool::Pact), n) => write!(f, "SLOT.PACT.{n}"),
            Self::SlotUsed(None, n) => write!(f, "SLOT.{n}.USED"),
            Self::SlotUsed(Some(SpellSlotPool::Arcane), n) => write!(f, "SLOT.ARCANE.{n}.USED"),
            Self::SlotUsed(Some(SpellSlotPool::Pact), n) => write!(f, "SLOT.PACT.{n}.USED"),
            Self::SlotPool => f.write_str("SLOT.POOL"),
            Self::CasterAbility => f.write_str("CASTER.ABILITY"),
            Self::CasterCoef => f.write_str("CASTER.COEF"),
            Self::SpellCantrips => f.write_str("SPELL.CANTRIPS"),
            Self::SpellKnown => f.write_str("SPELL.KNOWN"),
            Self::SpellReady => f.write_str("SPELL.READY"),
            Self::Arg(n) => write!(f, "ARG.{n}"),
            Self::Feature(name) => {
                if name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
                {
                    write!(f, "FEAT.{name}")
                } else {
                    write!(f, "FEAT.`{name}`")
                }
            }
            Self::Language(name) => {
                if name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
                {
                    write!(f, "LANG.{name}")
                } else {
                    write!(f, "LANG.`{name}`")
                }
            }
            Self::Species(name) => write!(f, "SPECIES.`{name}`"),
            Self::Background(name) => write!(f, "BACKGROUND.`{name}`"),
            Self::Subclass(name) => write!(f, "SUBCLASS.`{name}`"),
            Self::FeatCategory(cat) => write!(f, "FEAT_CAT.{cat}"),
        }
    }
}

impl Serialize for Attribute {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Attribute {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        <&str>::deserialize(de)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl Attribute {
    /// Human-readable translated name for this attribute.
    pub fn display_name(&self, i18n: I18n) -> String {
        match self {
            Self::Ability(a) => i18n.tr(a.tr_abbr_key()),
            Self::Modifier(a) => format!("{}.MOD", i18n.tr(a.tr_abbr_key())),
            Self::Skill(s) => i18n.tr(s.tr_key()),
            Self::SkillProficiency(s) => i18n.tr(s.tr_key()),
            Self::SavingThrow(a) => {
                format!(
                    "{} ({})",
                    tr!(i18n, "saving-throw"),
                    i18n.tr(a.tr_abbr_key())
                )
            }
            Self::SaveProficiency(a) => {
                format!(
                    "{} ({})",
                    tr!(i18n, "saving-throw"),
                    i18n.tr(a.tr_abbr_key())
                )
            }
            Self::EquipmentProficiency(p) => i18n.tr(p.tr_key()),
            Self::MaxHp => tr!(i18n, "hp-max"),
            Self::Speed => tr!(i18n, "speed"),
            Self::Initiative | Self::InitiativeBonus => tr!(i18n, "initiative"),
            Self::Ac => tr!(i18n, "armor-class"),
            Self::Inspiration => tr!(i18n, "inspiration"),
            Self::ProfBonus => tr!(i18n, "proficiency-bonus"),
            Self::Level => tr!(i18n, "level"),
            Self::ClassLevel(AttrKey::Scoped) => tr!(i18n, "class-level"),
            Self::ClassLevel(AttrKey::Named(name)) => {
                format!("{} ({name})", tr!(i18n, "class-level"))
            }
            Self::HitDice => tr!(i18n, "hit-dice"),
            Self::HitDiceMax => tr!(i18n, "hit-dice-max"),
            Self::HitDiceUsed => tr!(i18n, "hit-dice-used"),
            Self::HitDiceSides => tr!(i18n, "hit-dice-sides"),
            Self::CasterLevel(None) => tr!(i18n, "caster-level"),
            Self::CasterLevel(Some(pool)) => {
                format!("{} ({})", tr!(i18n, "caster-level"), i18n.tr(pool.tr_key()))
            }
            Self::Points(AttrKey::Scoped) => tr!(i18n, "points"),
            Self::Points(AttrKey::Named(name)) => format!("{} ({name})", tr!(i18n, "points")),
            Self::PointsMax(AttrKey::Scoped) => tr!(i18n, "points-max"),
            Self::PointsMax(AttrKey::Named(name)) => {
                format!("{} ({name})", tr!(i18n, "points-max"))
            }
            Self::DieSides(AttrKey::Named(name)) => {
                format!("{} ({name})", tr!(i18n, "die-sides"))
            }
            Self::DieCount(AttrKey::Named(name)) => {
                format!("{} ({name})", tr!(i18n, "die-count"))
            }
            Self::DieUsed(AttrKey::Named(name)) => {
                format!("{} ({name})", tr!(i18n, "die-used"))
            }
            Self::DieSides(AttrKey::Scoped) => tr!(i18n, "die-sides"),
            Self::DieCount(AttrKey::Scoped) => tr!(i18n, "die-count"),
            Self::DieUsed(AttrKey::Scoped) => tr!(i18n, "die-used"),
            Self::Bonus(AttrKey::Named(name)) => format!("{} ({name})", tr!(i18n, "bonus")),
            Self::Bonus(AttrKey::Scoped) => tr!(i18n, "bonus"),
            Self::ChoiceCount(AttrKey::Named(name)) => {
                format!("{} ({name})", tr!(i18n, "choice-count"))
            }
            Self::ChoiceCount(AttrKey::Scoped) => tr!(i18n, "choice-count"),
            Self::Sticky(AttrKey::Named(spell)) => format!("{} ({spell})", tr!(i18n, "sticky")),
            Self::Sticky(AttrKey::Scoped) => tr!(i18n, "sticky"),
            Self::FreeUses(AttrKey::Scoped) => tr!(i18n, "free-uses"),
            Self::FreeUses(AttrKey::Named(spell)) => {
                format!("{} ({spell})", tr!(i18n, "free-uses"))
            }
            Self::FreeUsesUsed(AttrKey::Scoped) => tr!(i18n, "free-uses-used"),
            Self::FreeUsesUsed(AttrKey::Named(spell)) => {
                format!("{} ({spell})", tr!(i18n, "free-uses-used"))
            }
            Self::Cost => tr!(i18n, "cost"),
            Self::Charges => tr!(i18n, "charges"),
            Self::ChargesMax => tr!(i18n, "charges-max"),
            Self::ChargesUsed => tr!(i18n, "charges-used"),
            Self::Quantity => tr!(i18n, "quantity"),
            Self::Resistance(dt) => {
                format!(
                    "{} ({})",
                    tr!(i18n, "damage-resistance"),
                    i18n.tr(dt.tr_key())
                )
            }
            Self::Vulnerability(dt) => {
                format!(
                    "{} ({})",
                    tr!(i18n, "damage-vulnerability"),
                    i18n.tr(dt.tr_key())
                )
            }
            Self::Immunity(dt) => {
                format!(
                    "{} ({})",
                    tr!(i18n, "damage-immunity"),
                    i18n.tr(dt.tr_key())
                )
            }
            Self::DamageReduction(dt) => {
                format!(
                    "{} ({})",
                    tr!(i18n, "damage-reduction"),
                    i18n.tr(dt.tr_key())
                )
            }
            Self::Attacks => tr!(i18n, "attack-count"),
            Self::Arg(_) => "?".to_string(),
            Self::Feature(name) => name.to_string(),
            Self::Language(name) => name.to_string(),
            Self::FeatCategory(cat) => i18n.tr(cat.tr_key()),
            Self::Slot(None, n) => format!("{} L{n}", tr!(i18n, "spell-slot")),
            Self::Slot(Some(pool), n) => format!(
                "{} L{n} ({})",
                tr!(i18n, "spell-slot"),
                i18n.tr(pool.tr_key())
            ),
            Self::SlotUsed(None, n) => format!("{} L{n}", tr!(i18n, "spell-slot-used")),
            Self::SlotUsed(Some(pool), n) => format!(
                "{} L{n} ({})",
                tr!(i18n, "spell-slot-used"),
                i18n.tr(pool.tr_key())
            ),
            Self::SlotPool => tr!(i18n, "spell-slot-pool"),
            Self::CasterAbility => tr!(i18n, "caster-ability"),
            Self::CasterCoef => tr!(i18n, "caster-coef"),
            Self::SpellCantrips => tr!(i18n, "spell-cantrips"),
            Self::SpellKnown => tr!(i18n, "spell-known"),
            Self::SpellReady => tr!(i18n, "spell-ready"),
            _ => self.to_string(),
        }
    }

    /// Format an `i32` value for this attribute. Most attrs render the
    /// number directly; CasterAbility/SlotPool/CasterCoef carry an enum
    /// index and should display its name.
    pub fn format_value(&self, value: i32, i18n: I18n) -> String {
        match self {
            Self::CasterAbility => Ability::try_from(value.max(0) as u8)
                .map(|ability| i18n.tr(ability.tr_abbr_key()))
                .unwrap_or_else(|_| value.to_string()),
            Self::SlotPool => SpellSlotPool::try_from(value.max(0) as u8)
                .map(|pool| i18n.tr(pool.tr_key()))
                .unwrap_or_else(|_| value.to_string()),
            Self::CasterCoef => match value {
                1 => tr!(i18n, "caster-coef-full"),
                2 => tr!(i18n, "caster-coef-half"),
                3 => tr!(i18n, "caster-coef-third"),
                _ => value.to_string(),
            },
            _ => value.to_string(),
        }
    }
}

impl Ability {
    pub fn abbr(self) -> &'static str {
        match self {
            Self::Strength => "STR",
            Self::Dexterity => "DEX",
            Self::Constitution => "CON",
            Self::Intelligence => "INT",
            Self::Wisdom => "WIS",
            Self::Charisma => "CHA",
        }
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;

    #[wasm_bindgen_test]
    fn parse_named_pool_optional_backticks() {
        // Backticks are optional when the name has no special chars.
        assert_eq!(
            "POINTS.Lucky.MAX".parse::<Attribute>().unwrap(),
            Attribute::PointsMax(AttrKey::named("Lucky"))
        );
        assert_eq!(
            "POINTS.Lucky".parse::<Attribute>().unwrap(),
            Attribute::Points(AttrKey::named("Lucky"))
        );
        assert_eq!(
            "DIE.Lucky.SIDES".parse::<Attribute>().unwrap(),
            Attribute::DieSides(AttrKey::named("Lucky"))
        );
        assert_eq!(
            "BONUS.Lucky".parse::<Attribute>().unwrap(),
            Attribute::Bonus(AttrKey::named("Lucky"))
        );
        assert_eq!(
            "STICKY.Fireball".parse::<Attribute>().unwrap(),
            Attribute::Sticky(AttrKey::named("Fireball"))
        );
        assert_eq!(
            "FREE_USES.Fireball.USED".parse::<Attribute>().unwrap(),
            Attribute::FreeUsesUsed(AttrKey::named("Fireball"))
        );
        // Backticked form for names with spaces still works.
        assert_eq!(
            "POINTS.`Sorcery Points`.MAX".parse::<Attribute>().unwrap(),
            Attribute::PointsMax(AttrKey::named("Sorcery Points"))
        );
    }

    #[wasm_bindgen_test]
    fn parse_gear_local_attributes() {
        assert_eq!("CHARGES".parse::<Attribute>().unwrap(), Attribute::Charges);
        assert_eq!(
            "CHARGES.MAX".parse::<Attribute>().unwrap(),
            Attribute::ChargesMax
        );
        assert_eq!(
            "CHARGES.USED".parse::<Attribute>().unwrap(),
            Attribute::ChargesUsed
        );
        assert_eq!(
            "QUANTITY".parse::<Attribute>().unwrap(),
            Attribute::Quantity
        );
    }

    #[wasm_bindgen_test]
    fn display_gear_local_round_trip() {
        for attr in [
            Attribute::Charges,
            Attribute::ChargesMax,
            Attribute::ChargesUsed,
            Attribute::Quantity,
        ] {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn parse_hit_dice_attributes() {
        assert_eq!("HIT_DICE".parse::<Attribute>().unwrap(), Attribute::HitDice);
        assert_eq!(
            "HIT_DICE.MAX".parse::<Attribute>().unwrap(),
            Attribute::HitDiceMax
        );
        assert_eq!(
            "HIT_DICE.USED".parse::<Attribute>().unwrap(),
            Attribute::HitDiceUsed
        );
        assert_eq!(
            "HIT_DICE.SIDES".parse::<Attribute>().unwrap(),
            Attribute::HitDiceSides
        );
    }

    #[wasm_bindgen_test]
    fn display_hit_dice_round_trip() {
        for attr in [
            Attribute::HitDice,
            Attribute::HitDiceMax,
            Attribute::HitDiceUsed,
            Attribute::HitDiceSides,
        ] {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn gear_local_attributes_not_scoped() {
        assert!(!Attribute::Charges.is_scoped());
        assert!(!Attribute::ChargesMax.is_scoped());
        assert!(!Attribute::ChargesUsed.is_scoped());
        assert!(!Attribute::Quantity.is_scoped());
    }

    #[wasm_bindgen_test]
    fn parse_advantage_attributes() {
        assert_eq!(
            "STR.ADV".parse::<Attribute>().unwrap(),
            Attribute::AbilityAdvantage(Ability::Strength)
        );
        assert_eq!(
            "CHA.ADV".parse::<Attribute>().unwrap(),
            Attribute::AbilityAdvantage(Ability::Charisma)
        );
        assert_eq!(
            "SKILL.STEA.ADV".parse::<Attribute>().unwrap(),
            Attribute::SkillAdvantage(Skill::Stealth)
        );
        assert_eq!(
            "SKILL.PERC.ADV".parse::<Attribute>().unwrap(),
            Attribute::SkillAdvantage(Skill::Perception)
        );
        assert_eq!(
            "DEX.SAVE.ADV".parse::<Attribute>().unwrap(),
            Attribute::SaveAdvantage(Ability::Dexterity)
        );
        assert_eq!(
            "WIS.SAVE.ADV".parse::<Attribute>().unwrap(),
            Attribute::SaveAdvantage(Ability::Wisdom)
        );
        assert_eq!(
            "ATK.ADV".parse::<Attribute>().unwrap(),
            Attribute::AttackAdvantage
        );
    }

    #[wasm_bindgen_test]
    fn display_advantage_round_trip() {
        let cases = [
            Attribute::AbilityAdvantage(Ability::Strength),
            Attribute::AbilityAdvantage(Ability::Charisma),
            Attribute::SkillAdvantage(Skill::Stealth),
            Attribute::SkillAdvantage(Skill::Perception),
            Attribute::SaveAdvantage(Ability::Dexterity),
            Attribute::SaveAdvantage(Ability::Wisdom),
            Attribute::AttackAdvantage,
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn is_advantage() {
        assert!(Attribute::AbilityAdvantage(Ability::Strength).is_advantage());
        assert!(Attribute::SkillAdvantage(Skill::Stealth).is_advantage());
        assert!(Attribute::SaveAdvantage(Ability::Dexterity).is_advantage());
        assert!(Attribute::AttackAdvantage.is_advantage());
        assert!(!Attribute::Ac.is_advantage());
        assert!(!Attribute::AttackBonus.is_advantage());
        assert!(!Attribute::Skill(Skill::Stealth).is_advantage());
    }

    #[wasm_bindgen_test]
    fn existing_attributes_still_parse() {
        assert_eq!(
            "STR".parse::<Attribute>().unwrap(),
            Attribute::Ability(Ability::Strength)
        );
        assert_eq!(
            "STR.MOD".parse::<Attribute>().unwrap(),
            Attribute::Modifier(Ability::Strength)
        );
        assert_eq!(
            "STR.SAVE".parse::<Attribute>().unwrap(),
            Attribute::SavingThrow(Ability::Strength)
        );
        assert_eq!(
            "SKILL.ACRO".parse::<Attribute>().unwrap(),
            Attribute::Skill(Skill::Acrobatics)
        );
        assert_eq!("ATK".parse::<Attribute>().unwrap(), Attribute::AttackBonus);
    }

    #[wasm_bindgen_test]
    fn parse_spell_attributes() {
        assert_eq!("SPELL.DC".parse::<Attribute>().unwrap(), Attribute::SpellDc);
        assert_eq!(
            "SPELL.ATK".parse::<Attribute>().unwrap(),
            Attribute::SpellAttack
        );
        assert_eq!(
            "SPELL.ATK.ADV".parse::<Attribute>().unwrap(),
            Attribute::SpellAttackAdvantage
        );
    }

    #[wasm_bindgen_test]
    fn display_spell_round_trip() {
        let cases = [
            Attribute::SpellDc,
            Attribute::SpellAttack,
            Attribute::SpellAttackAdvantage,
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn parse_slot_level() {
        assert_eq!(
            "SLOT.LEVEL".parse::<Attribute>().unwrap(),
            Attribute::SlotLevel
        );
        // Round-trip
        assert_eq!(Attribute::SlotLevel.to_string(), "SLOT.LEVEL");
    }

    #[wasm_bindgen_test]
    fn parse_points_attributes() {
        assert_eq!(
            "POINTS".parse::<Attribute>().unwrap(),
            Attribute::Points(AttrKey::Scoped)
        );
        assert_eq!(
            "POINTS.MAX".parse::<Attribute>().unwrap(),
            Attribute::PointsMax(AttrKey::Scoped)
        );
        assert_eq!(
            "POINTS.`Ki`".parse::<Attribute>().unwrap(),
            Attribute::Points(AttrKey::named("Ki"))
        );
        assert_eq!(
            "POINTS.`Ki`.MAX".parse::<Attribute>().unwrap(),
            Attribute::PointsMax(AttrKey::named("Ki"))
        );
    }

    #[wasm_bindgen_test]
    fn display_points_round_trip() {
        let cases = [
            Attribute::Points(AttrKey::Scoped),
            Attribute::PointsMax(AttrKey::Scoped),
            Attribute::Points(AttrKey::named("Ki")),
            Attribute::PointsMax(AttrKey::named("Ki")),
            Attribute::Points(AttrKey::named("Bardic Inspiration")),
            Attribute::PointsMax(AttrKey::named("Bardic Inspiration")),
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn parse_die_attributes() {
        assert_eq!(
            "DIE.`Sneak Attack`.SIDES".parse::<Attribute>().unwrap(),
            Attribute::DieSides(AttrKey::named("Sneak Attack"))
        );
        assert_eq!(
            "DIE.`Sneak Attack`.COUNT".parse::<Attribute>().unwrap(),
            Attribute::DieCount(AttrKey::named("Sneak Attack"))
        );
        assert_eq!(
            "DIE.`Sneak Attack`.USED".parse::<Attribute>().unwrap(),
            Attribute::DieUsed(AttrKey::named("Sneak Attack"))
        );
        assert!("DIE.`X`".parse::<Attribute>().is_err());
        assert!("DIE.`X`.BAD".parse::<Attribute>().is_err());
    }

    #[wasm_bindgen_test]
    fn display_die_round_trip() {
        let cases = [
            Attribute::DieSides(AttrKey::named("Sneak Attack")),
            Attribute::DieCount(AttrKey::named("Sneak Attack")),
            Attribute::DieUsed(AttrKey::named("Sneak Attack")),
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn parse_bonus_attributes() {
        assert_eq!(
            "BONUS.`Fighting Style`".parse::<Attribute>().unwrap(),
            Attribute::Bonus(AttrKey::named("Fighting Style"))
        );
        // Bonus has no suffix.
        assert!("BONUS.`X`.MAX".parse::<Attribute>().is_err());
    }

    #[wasm_bindgen_test]
    fn display_bonus_round_trip() {
        let attr = Attribute::Bonus(AttrKey::named("Fighting Style"));
        let s = attr.to_string();
        let parsed: Attribute = s.parse().unwrap();
        assert_eq!(parsed, attr, "round-trip failed for {s}");
    }

    #[wasm_bindgen_test]
    fn parse_choice_attributes() {
        assert_eq!(
            "CHOICE.`ASI`.COUNT".parse::<Attribute>().unwrap(),
            Attribute::ChoiceCount(AttrKey::named("ASI"))
        );
        assert!("CHOICE.`ASI`".parse::<Attribute>().is_err());
        assert!("CHOICE.`ASI`.PICK".parse::<Attribute>().is_err());
    }

    #[wasm_bindgen_test]
    fn display_choice_round_trip() {
        let attr = Attribute::ChoiceCount(AttrKey::named("ASI"));
        let s = attr.to_string();
        let parsed: Attribute = s.parse().unwrap();
        assert_eq!(parsed, attr, "round-trip failed for {s}");
    }

    #[wasm_bindgen_test]
    fn parse_sticky_attributes() {
        assert_eq!(
            "STICKY.`Mage Hand`".parse::<Attribute>().unwrap(),
            Attribute::Sticky(AttrKey::named("Mage Hand"))
        );
    }

    #[wasm_bindgen_test]
    fn parse_free_uses_attributes() {
        assert_eq!(
            "FREE_USES".parse::<Attribute>().unwrap(),
            Attribute::FreeUses(AttrKey::Scoped)
        );
        assert_eq!(
            "FREE_USES.USED".parse::<Attribute>().unwrap(),
            Attribute::FreeUsesUsed(AttrKey::Scoped)
        );
        assert_eq!(
            "FREE_USES.`Mage Hand`".parse::<Attribute>().unwrap(),
            Attribute::FreeUses(AttrKey::named("Mage Hand"))
        );
        assert_eq!(
            "FREE_USES.`Mage Hand`.USED".parse::<Attribute>().unwrap(),
            Attribute::FreeUsesUsed(AttrKey::named("Mage Hand"))
        );
    }

    #[wasm_bindgen_test]
    fn display_sticky_free_uses_round_trip() {
        let cases = [
            Attribute::Sticky(AttrKey::named("Mage Hand")),
            Attribute::FreeUses(AttrKey::Scoped),
            Attribute::FreeUses(AttrKey::named("Mage Hand")),
            Attribute::FreeUsesUsed(AttrKey::Scoped),
            Attribute::FreeUsesUsed(AttrKey::named("Mage Hand")),
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn invalid_advantage_attributes() {
        assert!("SKILL.STEA.MOD".parse::<Attribute>().is_err());
        assert!("ATK.MOD".parse::<Attribute>().is_err());
        assert!("STR.SAVE.MOD".parse::<Attribute>().is_err());
    }

    #[wasm_bindgen_test]
    fn parse_resistance_attributes() {
        assert_eq!(
            "RESIST.FIRE".parse::<Attribute>().unwrap(),
            Attribute::Resistance(DamageType::Fire)
        );
        assert_eq!(
            "VULN.COLD".parse::<Attribute>().unwrap(),
            Attribute::Vulnerability(DamageType::Cold)
        );
        assert_eq!(
            "IMMUNE.BLUDG".parse::<Attribute>().unwrap(),
            Attribute::Immunity(DamageType::Bludgeoning)
        );
        assert_eq!(
            "DR.FIRE".parse::<Attribute>().unwrap(),
            Attribute::DamageReduction(DamageType::Fire)
        );
    }

    #[wasm_bindgen_test]
    fn display_resistance_round_trip() {
        use strum::IntoEnumIterator;
        for dt in DamageType::iter() {
            for attr in [
                Attribute::Resistance(dt),
                Attribute::Vulnerability(dt),
                Attribute::Immunity(dt),
                Attribute::DamageReduction(dt),
            ] {
                let s = attr.to_string();
                let parsed: Attribute = s.parse().unwrap();
                assert_eq!(parsed, attr, "round-trip failed for {s}");
            }
        }
    }

    #[wasm_bindgen_test]
    fn parse_feature_attributes() {
        assert_eq!(
            "FEAT.`Alert`".parse::<Attribute>().unwrap(),
            Attribute::Feature(intern("Alert"))
        );
        assert_eq!(
            "FEAT.`War Caster`".parse::<Attribute>().unwrap(),
            Attribute::Feature(intern("War Caster"))
        );
        assert_eq!(
            "FEAT.`Spellcasting (Bard)`".parse::<Attribute>().unwrap(),
            Attribute::Feature(intern("Spellcasting (Bard)"))
        );
        assert_eq!(
            "FEAT.`Tinker's Magic`".parse::<Attribute>().unwrap(),
            Attribute::Feature(intern("Tinker's Magic"))
        );
        // Unquoted single-word names also work
        assert_eq!(
            "FEAT.Alert".parse::<Attribute>().unwrap(),
            Attribute::Feature(intern("Alert"))
        );
    }

    #[wasm_bindgen_test]
    fn display_feature_round_trip() {
        let cases = [
            Attribute::Feature(intern("Alert")),
            Attribute::Feature(intern("War Caster")),
            Attribute::Feature(intern("Spellcasting (Bard)")),
            Attribute::Feature(intern("Tinker's Magic")),
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn parse_feat_category_attributes() {
        assert_eq!(
            "FEAT_CAT.Dragonmark".parse::<Attribute>().unwrap(),
            Attribute::FeatCategory(FeatureCategory::Dragonmark)
        );
        assert_eq!(
            "FEAT_CAT.General".parse::<Attribute>().unwrap(),
            Attribute::FeatCategory(FeatureCategory::General)
        );
    }

    #[wasm_bindgen_test]
    fn display_feat_category_round_trip() {
        let cases = [
            Attribute::FeatCategory(FeatureCategory::Dragonmark),
            Attribute::FeatCategory(FeatureCategory::General),
            Attribute::FeatCategory(FeatureCategory::EpicBoon),
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn parse_slot_attributes() {
        assert_eq!(
            "SLOT.1".parse::<Attribute>().unwrap(),
            Attribute::Slot(None, 1)
        );
        assert_eq!(
            "SLOT.9".parse::<Attribute>().unwrap(),
            Attribute::Slot(None, 9)
        );
        assert_eq!(
            "SLOT.ARCANE.3".parse::<Attribute>().unwrap(),
            Attribute::Slot(Some(SpellSlotPool::Arcane), 3)
        );
        assert_eq!(
            "SLOT.PACT.5".parse::<Attribute>().unwrap(),
            Attribute::Slot(Some(SpellSlotPool::Pact), 5)
        );
        assert_eq!(
            "SLOT.1.USED".parse::<Attribute>().unwrap(),
            Attribute::SlotUsed(None, 1)
        );
        assert_eq!(
            "SLOT.PACT.5.USED".parse::<Attribute>().unwrap(),
            Attribute::SlotUsed(Some(SpellSlotPool::Pact), 5)
        );
        assert_eq!(
            "SLOT.POOL".parse::<Attribute>().unwrap(),
            Attribute::SlotPool
        );
    }

    #[wasm_bindgen_test]
    fn parse_slot_attributes_negative() {
        assert!("SLOT.0".parse::<Attribute>().is_err());
        assert!("SLOT.10".parse::<Attribute>().is_err());
        assert!("SLOT.BAD.1".parse::<Attribute>().is_err());
        assert!("SLOT.ARCANE.0".parse::<Attribute>().is_err());
        assert!("SLOT.ARCANE.10".parse::<Attribute>().is_err());
    }

    #[wasm_bindgen_test]
    fn parse_spell_count_attributes() {
        assert_eq!(
            "SPELL.CANTRIPS".parse::<Attribute>().unwrap(),
            Attribute::SpellCantrips
        );
        assert_eq!(
            "SPELL.KNOWN".parse::<Attribute>().unwrap(),
            Attribute::SpellKnown
        );
        assert_eq!(
            "SPELL.READY".parse::<Attribute>().unwrap(),
            Attribute::SpellReady
        );
    }

    #[wasm_bindgen_test]
    fn parse_caster_meta_attributes() {
        assert_eq!(
            "CASTER.ABILITY".parse::<Attribute>().unwrap(),
            Attribute::CasterAbility
        );
        assert_eq!(
            "CASTER.COEF".parse::<Attribute>().unwrap(),
            Attribute::CasterCoef
        );
    }

    #[wasm_bindgen_test]
    fn display_new_spell_attributes_round_trip() {
        let cases = [
            Attribute::Slot(None, 1),
            Attribute::Slot(None, 9),
            Attribute::Slot(Some(SpellSlotPool::Arcane), 3),
            Attribute::Slot(Some(SpellSlotPool::Pact), 5),
            Attribute::SlotUsed(None, 2),
            Attribute::SlotUsed(Some(SpellSlotPool::Arcane), 7),
            Attribute::SlotUsed(Some(SpellSlotPool::Pact), 5),
            Attribute::SlotPool,
            Attribute::SpellCantrips,
            Attribute::SpellKnown,
            Attribute::SpellReady,
            Attribute::CasterAbility,
            Attribute::CasterCoef,
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn parse_language_attributes() {
        assert_eq!(
            "LANG.Common".parse::<Attribute>().unwrap(),
            Attribute::Language(intern("Common"))
        );
        assert_eq!(
            "LANG.`Thieves' Cant`".parse::<Attribute>().unwrap(),
            Attribute::Language(intern("Thieves' Cant"))
        );
        assert_eq!(
            "LANG.Draconic".parse::<Attribute>().unwrap(),
            Attribute::Language(intern("Draconic"))
        );
    }

    #[wasm_bindgen_test]
    fn display_language_round_trip() {
        let cases = [
            Attribute::Language(intern("Common")),
            Attribute::Language(intern("Thieves' Cant")),
            Attribute::Language(intern("Draconic")),
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn parse_class_level_attributes() {
        assert_eq!(
            "CLASS.LEVEL".parse::<Attribute>().unwrap(),
            Attribute::ClassLevel(AttrKey::Scoped)
        );
        assert_eq!(
            "CLASS.`Wizard`.LEVEL".parse::<Attribute>().unwrap(),
            Attribute::ClassLevel(AttrKey::named("Wizard"))
        );
        assert_eq!(
            "CLASS.Wizard.LEVEL".parse::<Attribute>().unwrap(),
            Attribute::ClassLevel(AttrKey::named("Wizard"))
        );
        assert!("CLASS.`Wizard`".parse::<Attribute>().is_err());
        assert!("CLASS.`Wizard`.MAX".parse::<Attribute>().is_err());
    }

    #[wasm_bindgen_test]
    fn display_class_level_round_trip() {
        let cases = [
            Attribute::ClassLevel(AttrKey::Scoped),
            Attribute::ClassLevel(AttrKey::named("Wizard")),
            Attribute::ClassLevel(AttrKey::named("Cleric")),
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    #[wasm_bindgen_test]
    fn parse_identity_attributes() {
        assert_eq!(
            "SPECIES.`Human`".parse::<Attribute>().unwrap(),
            Attribute::Species(intern("Human"))
        );
        assert_eq!(
            "BACKGROUND.`Soldier`".parse::<Attribute>().unwrap(),
            Attribute::Background(intern("Soldier"))
        );
        assert_eq!(
            "SUBCLASS.`Life Domain`".parse::<Attribute>().unwrap(),
            Attribute::Subclass(intern("Life Domain"))
        );
    }

    #[wasm_bindgen_test]
    fn display_identity_round_trip() {
        let cases = [
            Attribute::Species(intern("Human")),
            Attribute::Species(intern("Half-Elf")),
            Attribute::Background(intern("Soldier")),
            Attribute::Background(intern("Folk Hero")),
            Attribute::Subclass(intern("Life Domain")),
            Attribute::Subclass(intern("College of Lore")),
        ];
        for attr in cases {
            let s = attr.to_string();
            let parsed: Attribute = s.parse().unwrap();
            assert_eq!(parsed, attr, "round-trip failed for {s}");
        }
    }

    /// Full expression parsing pipeline (tokenizer → parser → Attribute)
    /// for backtick-quoted feature names.
    #[wasm_bindgen_test]
    fn parse_expr_with_backtick_features() {
        use super::super::Expr;

        let expr: Expr = "LEVEL >= 4 and FEAT.`War Caster`".parse().unwrap();
        assert_eq!(expr.to_string(), "LEVEL >= 4 and FEAT.`War Caster`");

        let expr: Expr = "FEAT.`Spellcasting (Bard)` and not FEAT_CAT.Dragonmark"
            .parse()
            .unwrap();
        assert_eq!(
            expr.to_string(),
            "FEAT.`Spellcasting (Bard)` and not FEAT_CAT.Dragonmark"
        );

        // Unquoted works too
        let expr: Expr = "FEAT.Alert".parse().unwrap();
        assert_eq!(expr.to_string(), "FEAT.Alert");
    }
}
