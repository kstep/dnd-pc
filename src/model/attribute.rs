use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::model::{Ability, Proficiency, Skill};

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
    CasterLevel,
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

fn proficiency_name(p: Proficiency) -> &'static str {
    match p {
        Proficiency::LightArmor => "LIGHT_ARMOR",
        Proficiency::MediumArmor => "MEDIUM_ARMOR",
        Proficiency::HeavyArmor => "HEAVY_ARMOR",
        Proficiency::Shields => "SHIELDS",
        Proficiency::SimpleWeapons => "SIMPLE_WEAPONS",
        Proficiency::MartialWeapons => "MARTIAL_WEAPONS",
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
        // Dotted forms: STR.MOD, STR.SAVE, STR.ADV, STR.SAVE.ADV,
        // SKILL.ACRO, SKILL.ACRO.ADV, ATK.ADV
        if let Some((prefix, rest)) = s.split_once('.') {
            if prefix == "SKILL" {
                // SKILL.ACRO or SKILL.ACRO.ADV
                if let Some((skill_str, suffix)) = rest.split_once('.') {
                    return match suffix {
                        "ADV" => parse_skill(skill_str)
                            .map(Self::SkillAdvantage)
                            .ok_or("unknown skill"),
                        "PROF" => parse_skill(skill_str)
                            .map(Self::SkillProficiency)
                            .ok_or("unknown skill"),
                        _ => Err("unknown skill suffix (expected ADV or PROF)"),
                    };
                }
                return parse_skill(rest).map(Self::Skill).ok_or("unknown skill");
            }
            if prefix == "PROF" {
                return parse_proficiency(rest)
                    .map(Self::EquipmentProficiency)
                    .ok_or("unknown proficiency");
            }
            if prefix == "INITIATIVE" {
                return match rest {
                    "BONUS" => Ok(Self::InitiativeBonus),
                    _ => Err("unknown INITIATIVE suffix (expected BONUS)"),
                };
            }
            if prefix == "ATK" {
                return match rest {
                    "ADV" => Ok(Self::AttackAdvantage),
                    _ => Err("unknown ATK suffix (expected ADV)"),
                };
            }
            if prefix == "SPELL" {
                return match rest {
                    "DC" => Ok(Self::SpellDc),
                    "ATK" => Ok(Self::SpellAttack),
                    "ATK.ADV" => Ok(Self::SpellAttackAdvantage),
                    _ => Err("unknown SPELL suffix (expected DC, ATK, or ATK.ADV)"),
                };
            }
            if let Some(ability) = parse_ability(prefix) {
                // STR.MOD, STR.SAVE, STR.ADV, STR.SAVE.ADV
                if let Some((middle, suffix)) = rest.split_once('.') {
                    if middle == "SAVE" {
                        return match suffix {
                            "ADV" => Ok(Self::SaveAdvantage(ability)),
                            "PROF" => Ok(Self::SaveProficiency(ability)),
                            _ => Err("unknown SAVE suffix (expected ADV or PROF)"),
                        };
                    }
                    return Err("unknown ability suffix");
                }
                return match rest {
                    "MOD" => Ok(Self::Modifier(ability)),
                    "SAVE" => Ok(Self::SavingThrow(ability)),
                    "ADV" => Ok(Self::AbilityAdvantage(ability)),
                    _ => Err("unknown ability suffix (expected MOD, SAVE, or ADV)"),
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
            "ATK" => Ok(Self::AttackBonus),
            "INITIATIVE" => Ok(Self::Initiative),
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
            Self::SkillProficiency(skill) => write!(f, "SKILL.{}.PROF", skill_abbr(*skill)),
            Self::SaveProficiency(ability) => write!(f, "{}.SAVE.PROF", ability_abbr(*ability)),
            Self::EquipmentProficiency(prof) => write!(f, "PROF.{}", proficiency_name(*prof)),
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
            Self::AttackBonus => f.write_str("ATK"),
            Self::Initiative => f.write_str("INITIATIVE"),
            Self::InitiativeBonus => f.write_str("INITIATIVE.BONUS"),
            Self::Inspiration => f.write_str("INSPIRATION"),
            Self::AbilityAdvantage(ability) => write!(f, "{}.ADV", ability_abbr(*ability)),
            Self::SkillAdvantage(skill) => write!(f, "SKILL.{}.ADV", skill_abbr(*skill)),
            Self::SaveAdvantage(ability) => write!(f, "{}.SAVE.ADV", ability_abbr(*ability)),
            Self::AttackAdvantage => f.write_str("ATK.ADV"),
            Self::SpellDc => f.write_str("SPELL.DC"),
            Self::SpellAttack => f.write_str("SPELL.ATK"),
            Self::SpellAttackAdvantage => f.write_str("SPELL.ATK.ADV"),
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
    fn invalid_advantage_attributes() {
        assert!("SKILL.STEA.MOD".parse::<Attribute>().is_err());
        assert!("ATK.MOD".parse::<Attribute>().is_err());
        assert!("STR.SAVE.MOD".parse::<Attribute>().is_err());
    }
}
