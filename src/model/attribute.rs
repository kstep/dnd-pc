use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::model::{Ability, Skill};

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize
)]
pub enum Attribute {
    Ability(Ability),
    Modifier(Ability),
    SavingThrow(Ability),
    Skill(Skill),
    MaxHp,
    Hp,
    TempHp,
    Level,
    Ac,
    Speed,
    ClassLevel,
    CasterLevel,
    CasterModifier,
    ProfBonus,
    Inspiration,
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

fn skill_abbr(skill: Skill) -> &'static str {
    match skill {
        Skill::Acrobatics => "ACRO",
        Skill::AnimalHandling => "ANIM",
        Skill::Arcana => "ARCA",
        Skill::Athletics => "ATHL",
        Skill::Deception => "DECE",
        Skill::History => "HIST",
        Skill::Insight => "INSI",
        Skill::Intimidation => "INTI",
        Skill::Investigation => "INVE",
        Skill::Medicine => "MEDI",
        Skill::Nature => "NATU",
        Skill::Perception => "PERC",
        Skill::Performance => "PERF",
        Skill::Persuasion => "PERS",
        Skill::Religion => "RELI",
        Skill::SleightOfHand => "SLEI",
        Skill::Stealth => "STEA",
        Skill::Survival => "SURV",
    }
}

impl FromStr for Attribute {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Dotted forms: STR.MOD, STR.SAVE, SKILL.ACRO
        if let Some((prefix, suffix)) = s.split_once('.') {
            if prefix == "SKILL" {
                return parse_skill(suffix).map(Self::Skill).ok_or("unknown skill");
            }
            if let Some(ability) = parse_ability(prefix) {
                return match suffix {
                    "MOD" => Ok(Self::Modifier(ability)),
                    "SAVE" => Ok(Self::SavingThrow(ability)),
                    _ => Err("unknown ability suffix (expected MOD or SAVE)"),
                };
            }
            return Err("unknown attribute");
        }

        // Bare ability names => ability score
        if let Some(ability) = parse_ability(s) {
            return Ok(Self::Ability(ability));
        }

        match s {
            "MAX_HP" => Ok(Self::MaxHp),
            "HP" => Ok(Self::Hp),
            "TEMP_HP" => Ok(Self::TempHp),
            "LEVEL" => Ok(Self::Level),
            "AC" => Ok(Self::Ac),
            "SPEED" => Ok(Self::Speed),
            "CLASS_LEVEL" => Ok(Self::ClassLevel),
            "CASTER_LEVEL" => Ok(Self::CasterLevel),
            "CASTER_MODIFIER" => Ok(Self::CasterModifier),
            "PROF_BONUS" => Ok(Self::ProfBonus),
            "INSPIRATION" => Ok(Self::Inspiration),
            _ => Err("unknown attribute"),
        }
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Ability(ability) => write!(f, "{}", ability_abbr(*ability)),
            Self::Modifier(ability) => write!(f, "{}.MOD", ability_abbr(*ability)),
            Self::SavingThrow(ability) => write!(f, "{}.SAVE", ability_abbr(*ability)),
            Self::Skill(skill) => write!(f, "SKILL.{}", skill_abbr(*skill)),
            Self::MaxHp => f.write_str("MAX_HP"),
            Self::Hp => f.write_str("HP"),
            Self::TempHp => f.write_str("TEMP_HP"),
            Self::Level => f.write_str("LEVEL"),
            Self::Ac => f.write_str("AC"),
            Self::Speed => f.write_str("SPEED"),
            Self::ClassLevel => f.write_str("CLASS_LEVEL"),
            Self::CasterLevel => f.write_str("CASTER_LEVEL"),
            Self::CasterModifier => f.write_str("CASTER_MODIFIER"),
            Self::ProfBonus => f.write_str("PROF_BONUS"),
            Self::Inspiration => f.write_str("INSPIRATION"),
        }
    }
}

fn ability_abbr(ability: Ability) -> &'static str {
    match ability {
        Ability::Strength => "STR",
        Ability::Dexterity => "DEX",
        Ability::Constitution => "CON",
        Ability::Intelligence => "INT",
        Ability::Wisdom => "WIS",
        Ability::Charisma => "CHA",
    }
}
