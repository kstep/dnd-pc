use std::{fmt, str::FromStr};

use serde::Deserialize;

use crate::model::Ability;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub enum Attribute {
    Modifier(Ability),
    MaxHp,
    Hp,
    TempHp,
    Level,
    Ac,
    Speed,
    ClassLevel,
    CasterLevel,
    CasterModifier,
    Inspiration,
}

impl FromStr for Attribute {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "STR" => Ok(Self::Modifier(Ability::Strength)),
            "DEX" => Ok(Self::Modifier(Ability::Dexterity)),
            "CON" => Ok(Self::Modifier(Ability::Constitution)),
            "INT" => Ok(Self::Modifier(Ability::Intelligence)),
            "WIS" => Ok(Self::Modifier(Ability::Wisdom)),
            "CHA" => Ok(Self::Modifier(Ability::Charisma)),
            "MAX_HP" => Ok(Self::MaxHp),
            "HP" => Ok(Self::Hp),
            "TEMP_HP" => Ok(Self::TempHp),
            "LEVEL" => Ok(Self::Level),
            "AC" => Ok(Self::Ac),
            "SPEED" => Ok(Self::Speed),
            "CLASS_LEVEL" => Ok(Self::ClassLevel),
            "CASTER_LEVEL" => Ok(Self::CasterLevel),
            "CASTER_MODIFIER" => Ok(Self::CasterModifier),
            "INSPIRATION" => Ok(Self::Inspiration),
            _ => Err("unknown attribute"),
        }
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::Modifier(Ability::Strength) => "STR",
            Self::Modifier(Ability::Dexterity) => "DEX",
            Self::Modifier(Ability::Constitution) => "CON",
            Self::Modifier(Ability::Intelligence) => "INT",
            Self::Modifier(Ability::Wisdom) => "WIS",
            Self::Modifier(Ability::Charisma) => "CHA",
            Self::CasterModifier => "CASTER_MODIFIER",
            Self::MaxHp => "MAX_HP",
            Self::Hp => "HP",
            Self::TempHp => "TEMP_HP",
            Self::Level => "LEVEL",
            Self::Ac => "AC",
            Self::Speed => "SPEED",
            Self::ClassLevel => "CLASS_LEVEL",
            Self::CasterLevel => "CASTER_LEVEL",
            Self::Inspiration => "INSPIRATION",
        })
    }
}
