use std::{cell::RefCell, collections::HashSet, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::model::{
    Ability, DamageType, FeatureCategory, Proficiency, Skill, SpellSlotPool, Translatable,
};

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
    ClassLevel,
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
    Points(u8),
    PointsMax(u8),
    Cost,
    Resistance(DamageType),
    Vulnerability(DamageType),
    Immunity(DamageType),
    DamageReduction(DamageType),
    Attacks,
    Arg(u8),
    Feature(&'static str),
    FeatCategory(FeatureCategory),
    Language(&'static str),
}

/// Intern a string for the lifetime of the program.
/// Intentional leak — used for Feature attribute names that live
/// until the wasm instance (browser tab) is closed. Deduplicates
/// via a global HashSet so each unique name is leaked at most once.
fn intern(s: &str) -> &'static str {
    thread_local! {
        static INTERNED: RefCell<HashSet<&'static str>> = RefCell::new(HashSet::new());
    }
    INTERNED.with_borrow_mut(|set| {
        if let Some(&existing) = set.get(s) {
            return existing;
        }
        let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
        set.insert(leaked);
        leaked
    })
}

impl Attribute {
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
            Self::SpellDc | Self::SpellAttack | Self::SpellAttackAdvantage
        )
    }
}

fn parse_ability(s: &str) -> Option<Ability> {
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

fn parse_damage_type(s: &str) -> Option<DamageType> {
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

fn parse_skill(s: &str) -> Option<Skill> {
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

impl FromStr for Attribute {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Dotted forms: STR.MOD, STR.SAVE, STR.ADV, STR.SAVE.ADV,
        // SKILL.ACRO, SKILL.ACRO.ADV, ATK.ADV
        let Some((prefix, rest)) = s.split_once('.') else {
            return match s {
                "MAX_HP" => Ok(Self::MaxHp),
                "HP" => Ok(Self::Hp),
                "TEMP_HP" => Ok(Self::TempHp),
                "LEVEL" => Ok(Self::Level),
                "AC" => Ok(Self::Ac),
                "SPEED" => Ok(Self::Speed),
                "CLASS_LEVEL" => Ok(Self::ClassLevel),
                "CASTER_LEVEL" => Ok(Self::CasterLevel(None)),
                "CASTER_MODIFIER" => Ok(Self::CasterModifier),
                "PROF_BONUS" => Ok(Self::ProfBonus),
                "ATK" => Ok(Self::AttackBonus),
                "INITIATIVE" => Ok(Self::Initiative),
                "INSPIRATION" => Ok(Self::Inspiration),
                "ATTACKS" => Ok(Self::Attacks),
                "SLOT_LEVEL" => Ok(Self::SlotLevel),
                "POINTS" => Ok(Self::Points(0)),
                "POINTS_MAX" => Ok(Self::PointsMax(0)),
                "COST" => Ok(Self::Cost),
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
            "PROF" => parse_proficiency(rest)
                .map(Self::EquipmentProficiency)
                .ok_or("unknown proficiency"),
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
            "POINTS" => rest
                .parse::<u8>()
                .map(Self::Points)
                .map_err(|_| "invalid POINTS index (expected integer 0-255)"),
            "POINTS_MAX" => rest
                .parse::<u8>()
                .map(Self::PointsMax)
                .map_err(|_| "invalid POINTS_MAX index (expected integer 0-255)"),
            "INITIATIVE" => match rest {
                "BONUS" => Ok(Self::InitiativeBonus),
                _ => Err("unknown INITIATIVE suffix (expected BONUS)"),
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
                _ => Err("unknown SPELL suffix (expected DC, ATK, or ATK.ADV)"),
            },
            "CASTER_LEVEL" => match rest {
                "ARCANE" => Ok(Self::CasterLevel(Some(SpellSlotPool::Arcane))),
                "PACT" => Ok(Self::CasterLevel(Some(SpellSlotPool::Pact))),
                _ => Err("unknown CASTER_LEVEL suffix (expected ARCANE or PACT)"),
            },
            "FEAT" => {
                let name = rest.trim_matches('`');
                Ok(Self::Feature(intern(name)))
            }
            "LANG" => {
                let name = rest.trim_matches('`');
                Ok(Self::Language(intern(name)))
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
            Self::MaxHp => f.write_str("MAX_HP"),
            Self::Hp => f.write_str("HP"),
            Self::TempHp => f.write_str("TEMP_HP"),
            Self::Level => f.write_str("LEVEL"),
            Self::Ac => f.write_str("AC"),
            Self::Speed => f.write_str("SPEED"),
            Self::ClassLevel => f.write_str("CLASS_LEVEL"),
            Self::CasterLevel(None) => f.write_str("CASTER_LEVEL"),
            Self::CasterLevel(Some(SpellSlotPool::Arcane)) => f.write_str("CASTER_LEVEL.ARCANE"),
            Self::CasterLevel(Some(SpellSlotPool::Pact)) => f.write_str("CASTER_LEVEL.PACT"),
            Self::CasterModifier => f.write_str("CASTER_MODIFIER"),
            Self::ProfBonus => f.write_str("PROF_BONUS"),
            Self::AttackBonus => f.write_str("ATK"),
            Self::Initiative => f.write_str("INITIATIVE"),
            Self::InitiativeBonus => f.write_str("INITIATIVE.BONUS"),
            Self::Inspiration => f.write_str("INSPIRATION"),
            Self::AbilityAdvantage(ability) => write!(f, "{}.ADV", ability.abbr()),
            Self::SkillAdvantage(skill) => write!(f, "SKILL.{}.ADV", skill.abbr()),
            Self::SaveAdvantage(ability) => write!(f, "{}.SAVE.ADV", ability.abbr()),
            Self::AttackAdvantage => f.write_str("ATK.ADV"),
            Self::SpellDc => f.write_str("SPELL.DC"),
            Self::SpellAttack => f.write_str("SPELL.ATK"),
            Self::SpellAttackAdvantage => f.write_str("SPELL.ATK.ADV"),
            Self::SlotLevel => f.write_str("SLOT_LEVEL"),
            Self::Points(0) => f.write_str("POINTS"),
            Self::Points(n) => write!(f, "POINTS.{n}"),
            Self::PointsMax(0) => f.write_str("POINTS_MAX"),
            Self::PointsMax(n) => write!(f, "POINTS_MAX.{n}"),
            Self::Cost => f.write_str("COST"),
            Self::Resistance(dt) => write!(f, "RESIST.{}", dt.abbr()),
            Self::Vulnerability(dt) => write!(f, "VULN.{}", dt.abbr()),
            Self::Immunity(dt) => write!(f, "IMMUNE.{}", dt.abbr()),
            Self::DamageReduction(dt) => write!(f, "DR.{}", dt.abbr()),
            Self::Attacks => f.write_str("ATTACKS"),
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
    pub fn display_name(&self, i18n: &leptos_fluent::I18n) -> String {
        match self {
            Self::Ability(a) | Self::Modifier(a) => i18n.tr(a.tr_abbr_key()),
            Self::Skill(s) | Self::SkillProficiency(s) => i18n.tr(s.tr_key()),
            Self::SavingThrow(a) | Self::SaveProficiency(a) => i18n.tr(a.tr_abbr_key()),
            Self::EquipmentProficiency(p) => i18n.tr(p.tr_key()),
            Self::MaxHp => i18n.tr("hp-max"),
            Self::Speed => i18n.tr("speed"),
            Self::Initiative | Self::InitiativeBonus => i18n.tr("initiative"),
            Self::Ac => i18n.tr("armor-class"),
            Self::Inspiration => i18n.tr("inspiration"),
            Self::ProfBonus => i18n.tr("proficiency-bonus"),
            Self::Level => i18n.tr("level"),
            Self::ClassLevel => i18n.tr("class-level"),
            Self::CasterLevel(None) => i18n.tr("caster-level"),
            Self::CasterLevel(Some(pool)) => {
                format!("{} ({})", i18n.tr("caster-level"), i18n.tr(pool.tr_key()))
            }
            Self::Points(_) => i18n.tr("points"),
            Self::PointsMax(_) => i18n.tr("points-max"),
            Self::Cost => i18n.tr("cost"),
            Self::Resistance(dt) => {
                format!(
                    "{} ({})",
                    i18n.tr("damage-resistance"),
                    i18n.tr(dt.tr_key())
                )
            }
            Self::Vulnerability(dt) => {
                format!(
                    "{} ({})",
                    i18n.tr("damage-vulnerability"),
                    i18n.tr(dt.tr_key())
                )
            }
            Self::Immunity(dt) => {
                format!("{} ({})", i18n.tr("damage-immunity"), i18n.tr(dt.tr_key()))
            }
            Self::DamageReduction(dt) => {
                format!("{} ({})", i18n.tr("damage-reduction"), i18n.tr(dt.tr_key()))
            }
            Self::Attacks => i18n.tr("attack-count"),
            Self::Arg(_) => "?".to_string(),
            Self::Feature(name) => name.to_string(),
            Self::Language(name) => name.to_string(),
            Self::FeatCategory(cat) => i18n.tr(cat.tr_key()),
            _ => self.to_string(),
        }
    }
}

impl Ability {
    fn abbr(self) -> &'static str {
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
            "SLOT_LEVEL".parse::<Attribute>().unwrap(),
            Attribute::SlotLevel
        );
        // Round-trip
        assert_eq!(Attribute::SlotLevel.to_string(), "SLOT_LEVEL");
    }

    #[wasm_bindgen_test]
    fn parse_points_attributes() {
        assert_eq!("POINTS".parse::<Attribute>().unwrap(), Attribute::Points(0));
        assert_eq!(
            "POINTS.2".parse::<Attribute>().unwrap(),
            Attribute::Points(2)
        );
        assert_eq!(
            "POINTS_MAX".parse::<Attribute>().unwrap(),
            Attribute::PointsMax(0)
        );
        assert_eq!(
            "POINTS_MAX.1".parse::<Attribute>().unwrap(),
            Attribute::PointsMax(1)
        );
    }

    #[wasm_bindgen_test]
    fn display_points_round_trip() {
        let cases = [
            Attribute::Points(0),
            Attribute::Points(3),
            Attribute::PointsMax(0),
            Attribute::PointsMax(2),
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

    /// Full expression parsing pipeline (tokenizer → parser → Attribute)
    /// for backtick-quoted feature names.
    #[wasm_bindgen_test]
    fn parse_expr_with_backtick_features() {
        use crate::expr::Expr;

        let expr: Expr<Attribute, i32> = "LEVEL >= 4 and FEAT.`War Caster`".parse().unwrap();
        assert_eq!(expr.to_string(), "LEVEL >= 4 and FEAT.`War Caster`");

        let expr: Expr<Attribute, i32> = "FEAT.`Spellcasting (Bard)` and not FEAT_CAT.Dragonmark"
            .parse()
            .unwrap();
        assert_eq!(
            expr.to_string(),
            "FEAT.`Spellcasting (Bard)` and not FEAT_CAT.Dragonmark"
        );

        // Unquoted works too
        let expr: Expr<Attribute, i32> = "FEAT.Alert".parse().unwrap();
        assert_eq!(expr.to_string(), "FEAT.Alert");
    }
}
