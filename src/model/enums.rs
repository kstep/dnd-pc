use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};

pub trait Translatable {
    fn tr_key(&self) -> &'static str;
    fn tr_abbr_key(&self) -> &'static str {
        self.tr_key()
    }
}

/// Implements Serialize (as u8) and Deserialize (u8 or legacy string name) for
/// a `#[repr(u8)]` enum.
macro_rules! enum_serde_u8 {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_u8(*self as u8)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct Vis;
                impl serde::de::Visitor<'_> for Vis {
                    type Value = $name;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str(concat!("u8 or string for ", stringify!($name)))
                    }

                    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<$name, E> {
                        $(if v == $name::$variant as u64 {
                            return Ok($name::$variant);
                        })+
                        Err(E::invalid_value(serde::de::Unexpected::Unsigned(v), &self))
                    }

                    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<$name, E> {
                        match v {
                            $(stringify!($variant) => Ok($name::$variant),)+
                            _ => Err(E::invalid_value(serde::de::Unexpected::Str(v), &self)),
                        }
                    }
                }
                d.deserialize_any(Vis)
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Display)]
#[repr(u8)]
pub enum Ability {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
}
enum_serde_u8!(Ability {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Display)]
#[repr(u8)]
pub enum Skill {
    Acrobatics,
    #[strum(serialize = "Animal Handling")]
    AnimalHandling,
    Arcana,
    Athletics,
    Deception,
    History,
    Insight,
    Intimidation,
    Investigation,
    Medicine,
    Nature,
    Perception,
    Performance,
    Persuasion,
    Religion,
    #[strum(serialize = "Sleight of Hand")]
    SleightOfHand,
    Stealth,
    Survival,
}
enum_serde_u8!(Skill {
    Acrobatics,
    AnimalHandling,
    Arcana,
    Athletics,
    Deception,
    History,
    Insight,
    Intimidation,
    Investigation,
    Medicine,
    Nature,
    Perception,
    Performance,
    Persuasion,
    Religion,
    SleightOfHand,
    Stealth,
    Survival,
});

impl Skill {
    pub fn ability(self) -> Ability {
        match self {
            Skill::Athletics => Ability::Strength,
            Skill::Acrobatics | Skill::SleightOfHand | Skill::Stealth => Ability::Dexterity,
            Skill::Arcana
            | Skill::History
            | Skill::Investigation
            | Skill::Nature
            | Skill::Religion => Ability::Intelligence,
            Skill::AnimalHandling
            | Skill::Insight
            | Skill::Medicine
            | Skill::Perception
            | Skill::Survival => Ability::Wisdom,
            Skill::Deception | Skill::Intimidation | Skill::Performance | Skill::Persuasion => {
                Ability::Charisma
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter)]
#[repr(u8)]
pub enum Alignment {
    #[strum(serialize = "Lawful Good")]
    LawfulGood,
    #[strum(serialize = "Neutral Good")]
    NeutralGood,
    #[strum(serialize = "Chaotic Good")]
    ChaoticGood,
    #[strum(serialize = "Lawful Neutral")]
    LawfulNeutral,
    #[strum(serialize = "True Neutral")]
    TrueNeutral,
    #[strum(serialize = "Chaotic Neutral")]
    ChaoticNeutral,
    #[strum(serialize = "Lawful Evil")]
    LawfulEvil,
    #[strum(serialize = "Neutral Evil")]
    NeutralEvil,
    #[strum(serialize = "Chaotic Evil")]
    ChaoticEvil,
}
enum_serde_u8!(Alignment {
    LawfulGood,
    NeutralGood,
    ChaoticGood,
    LawfulNeutral,
    TrueNeutral,
    ChaoticNeutral,
    LawfulEvil,
    NeutralEvil,
    ChaoticEvil,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProficiencyLevel {
    None,
    Proficient,
    Expertise,
}
enum_serde_u8!(ProficiencyLevel {
    None,
    Proficient,
    Expertise
});

impl ProficiencyLevel {
    pub fn multiplier(self) -> i32 {
        match self {
            ProficiencyLevel::None => 0,
            ProficiencyLevel::Proficient => 1,
            ProficiencyLevel::Expertise => 2,
        }
    }

    pub fn next(self) -> Self {
        match self {
            ProficiencyLevel::None => ProficiencyLevel::Proficient,
            ProficiencyLevel::Proficient => ProficiencyLevel::Expertise,
            ProficiencyLevel::Expertise => ProficiencyLevel::None,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            ProficiencyLevel::None => "\u{25CB}",       // empty circle
            ProficiencyLevel::Proficient => "\u{25CF}", // filled circle
            ProficiencyLevel::Expertise => "\u{25C9}",  // fisheye (double)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Display)]
#[repr(u8)]
pub enum Proficiency {
    #[strum(serialize = "Light Armor")]
    LightArmor,
    #[strum(serialize = "Medium Armor")]
    MediumArmor,
    #[strum(serialize = "Heavy Armor")]
    HeavyArmor,
    Shields,
    #[strum(serialize = "Simple Weapons")]
    SimpleWeapons,
    #[strum(serialize = "Martial Weapons")]
    MartialWeapons,
}
enum_serde_u8!(Proficiency {
    LightArmor,
    MediumArmor,
    HeavyArmor,
    Shields,
    SimpleWeapons,
    MartialWeapons,
});

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter)]
#[repr(u8)]
pub enum DamageType {
    Acid,
    Bludgeoning,
    Cold,
    Fire,
    Force,
    Lightning,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Radiant,
    Slashing,
    Thunder,
}
enum_serde_u8!(DamageType {
    Acid,
    Bludgeoning,
    Cold,
    Fire,
    Force,
    Lightning,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Radiant,
    Slashing,
    Thunder,
});

impl Translatable for Ability {
    fn tr_key(&self) -> &'static str {
        match self {
            Ability::Strength => "ability-strength",
            Ability::Dexterity => "ability-dexterity",
            Ability::Constitution => "ability-constitution",
            Ability::Intelligence => "ability-intelligence",
            Ability::Wisdom => "ability-wisdom",
            Ability::Charisma => "ability-charisma",
        }
    }

    fn tr_abbr_key(&self) -> &'static str {
        match self {
            Ability::Strength => "ability-str",
            Ability::Dexterity => "ability-dex",
            Ability::Constitution => "ability-con",
            Ability::Intelligence => "ability-int",
            Ability::Wisdom => "ability-wis",
            Ability::Charisma => "ability-cha",
        }
    }
}

impl Translatable for Skill {
    fn tr_key(&self) -> &'static str {
        match self {
            Skill::Acrobatics => "skill-acrobatics",
            Skill::AnimalHandling => "skill-animal-handling",
            Skill::Arcana => "skill-arcana",
            Skill::Athletics => "skill-athletics",
            Skill::Deception => "skill-deception",
            Skill::History => "skill-history",
            Skill::Insight => "skill-insight",
            Skill::Intimidation => "skill-intimidation",
            Skill::Investigation => "skill-investigation",
            Skill::Medicine => "skill-medicine",
            Skill::Nature => "skill-nature",
            Skill::Perception => "skill-perception",
            Skill::Performance => "skill-performance",
            Skill::Persuasion => "skill-persuasion",
            Skill::Religion => "skill-religion",
            Skill::SleightOfHand => "skill-sleight-of-hand",
            Skill::Stealth => "skill-stealth",
            Skill::Survival => "skill-survival",
        }
    }
}

impl Translatable for Alignment {
    fn tr_key(&self) -> &'static str {
        match self {
            Alignment::LawfulGood => "alignment-lawful-good",
            Alignment::NeutralGood => "alignment-neutral-good",
            Alignment::ChaoticGood => "alignment-chaotic-good",
            Alignment::LawfulNeutral => "alignment-lawful-neutral",
            Alignment::TrueNeutral => "alignment-true-neutral",
            Alignment::ChaoticNeutral => "alignment-chaotic-neutral",
            Alignment::LawfulEvil => "alignment-lawful-evil",
            Alignment::NeutralEvil => "alignment-neutral-evil",
            Alignment::ChaoticEvil => "alignment-chaotic-evil",
        }
    }
}

impl Translatable for Proficiency {
    fn tr_key(&self) -> &'static str {
        match self {
            Proficiency::LightArmor => "prof-light-armor",
            Proficiency::MediumArmor => "prof-medium-armor",
            Proficiency::HeavyArmor => "prof-heavy-armor",
            Proficiency::Shields => "prof-shields",
            Proficiency::SimpleWeapons => "prof-simple-weapons",
            Proficiency::MartialWeapons => "prof-martial-weapons",
        }
    }
}
