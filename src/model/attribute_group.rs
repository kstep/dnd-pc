use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use strum::VariantArray;

use crate::{
    expr::{IterIndex, VarGroup},
    model::{
        Ability, Attribute, DamageType, Skill,
        attribute::{parse_ability, parse_damage_type, parse_skill},
    },
};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeGroup {
    #[default]
    Ability,
    AbilityMod,
    AbilitySave,
    AbilitySaveProf,
    AbilitySaveAdv,
    AbilityAdv,
    Skill,
    SkillProf,
    SkillAdv,
    Resist,
    Vuln,
    Immune,
    Arg,
}

impl VarGroup for AttributeGroup {
    type Var = Attribute;

    fn member_by_name(&self, name: &str) -> Option<usize> {
        match self {
            Self::Ability
            | Self::AbilityMod
            | Self::AbilitySave
            | Self::AbilitySaveProf
            | Self::AbilitySaveAdv
            | Self::AbilityAdv => {
                let ability = parse_ability(name)?;
                Ability::VARIANTS.iter().position(|&a| a == ability)
            }
            Self::Skill | Self::SkillProf | Self::SkillAdv => {
                let skill = parse_skill(name)?;
                Skill::VARIANTS.iter().position(|&s| s == skill)
            }
            Self::Resist | Self::Vuln | Self::Immune => {
                let dmg = parse_damage_type(name)?;
                DamageType::VARIANTS.iter().position(|&d| d == dmg)
            }
            Self::Arg => name.parse::<usize>().ok(),
        }
    }

    fn member(&self, idx: IterIndex) -> Option<Attribute> {
        let i = idx.index;
        let ability = || Ability::VARIANTS.get(i).copied();
        let skill = || Skill::VARIANTS.get(i).copied();
        let dmg = || DamageType::VARIANTS.get(i).copied();
        match self {
            Self::Ability => ability().map(Attribute::Ability),
            Self::AbilityMod => ability().map(Attribute::Modifier),
            Self::AbilitySave => ability().map(Attribute::SavingThrow),
            Self::AbilitySaveProf => ability().map(Attribute::SaveProficiency),
            Self::AbilitySaveAdv => ability().map(Attribute::SaveAdvantage),
            Self::AbilityAdv => ability().map(Attribute::AbilityAdvantage),
            Self::Skill => skill().map(Attribute::Skill),
            Self::SkillProf => skill().map(Attribute::SkillProficiency),
            Self::SkillAdv => skill().map(Attribute::SkillAdvantage),
            Self::Resist => dmg().map(Attribute::Resistance),
            Self::Vuln => dmg().map(Attribute::Vulnerability),
            Self::Immune => dmg().map(Attribute::Immunity),
            Self::Arg => Some(Attribute::Arg(idx.iter_no as u8)),
        }
    }
}

impl FromStr for AttributeGroup {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ABILITY" => Ok(Self::Ability),
            "ABILITY.MOD" => Ok(Self::AbilityMod),
            "ABILITY.SAVE" => Ok(Self::AbilitySave),
            "ABILITY.SAVE.PROF" => Ok(Self::AbilitySaveProf),
            "ABILITY.SAVE.ADV" => Ok(Self::AbilitySaveAdv),
            "ABILITY.ADV" => Ok(Self::AbilityAdv),
            "SKILL._" => Ok(Self::Skill),
            "SKILL._.PROF" => Ok(Self::SkillProf),
            "SKILL._.ADV" => Ok(Self::SkillAdv),
            "RESIST._" => Ok(Self::Resist),
            "VULN._" => Ok(Self::Vuln),
            "IMMUNE._" => Ok(Self::Immune),
            "ARG" => Ok(Self::Arg),
            _ => Err("unknown attribute group"),
        }
    }
}

impl fmt::Display for AttributeGroup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::Ability => "ABILITY",
            Self::AbilityMod => "ABILITY.MOD",
            Self::AbilitySave => "ABILITY.SAVE",
            Self::AbilitySaveProf => "ABILITY.SAVE.PROF",
            Self::AbilitySaveAdv => "ABILITY.SAVE.ADV",
            Self::AbilityAdv => "ABILITY.ADV",
            Self::Skill => "SKILL._",
            Self::SkillProf => "SKILL._.PROF",
            Self::SkillAdv => "SKILL._.ADV",
            Self::Resist => "RESIST._",
            Self::Vuln => "VULN._",
            Self::Immune => "IMMUNE._",
            Self::Arg => "ARG",
        })
    }
}
