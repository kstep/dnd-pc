use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

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
        impl TryFrom<u8> for $name {
            type Error = u8;

            fn try_from(n: u8) -> Result<Self, Self::Error> {
                $(if n == Self::$variant as u8 {
                    return Ok(Self::$variant);
                })+
                Err(n)
            }
        }

        impl $name {
            pub fn from_u8_str(s: &str) -> Option<Self> {
                s.parse::<u8>().ok().and_then(|n| Self::try_from(n).ok())
            }
        }

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
                        let Ok(n) = u8::try_from(v) else {
                            return Err(E::invalid_value(serde::de::Unexpected::Unsigned(v), &self));
                        };
                        $name::try_from(n)
                            .map_err(|_| E::invalid_value(serde::de::Unexpected::Unsigned(v), &self))
                    }

                    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<$name, E> {
                        $name::from_u8_str(v)
                            .or_else(|| match v {
                                $(stringify!($variant) => Some($name::$variant),)+
                                _ => None,
                            })
                            .ok_or_else(|| E::invalid_value(serde::de::Unexpected::Str(v), &self))
                    }
                }
                d.deserialize_u8(Vis)
            }
        }
    };
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Display, PartialOrd, Ord
)]
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Display, EnumString, PartialOrd, Ord
)]
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
            Self::Athletics => Ability::Strength,
            Self::Acrobatics | Self::SleightOfHand | Self::Stealth => Ability::Dexterity,
            Self::Arcana | Self::History | Self::Investigation | Self::Nature | Self::Religion => {
                Ability::Intelligence
            }
            Self::AnimalHandling
            | Self::Insight
            | Self::Medicine
            | Self::Perception
            | Self::Survival => Ability::Wisdom,
            Self::Deception | Self::Intimidation | Self::Performance | Self::Persuasion => {
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
    pub fn is_proficient(self) -> bool {
        self != Self::None
    }

    pub fn multiplier(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Proficient => 1,
            Self::Expertise => 2,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::None => Self::Proficient,
            Self::Proficient => Self::Expertise,
            Self::Expertise => Self::None,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::None => "\u{25CB}",       // empty circle
            Self::Proficient => "\u{25CF}", // filled circle
            Self::Expertise => "\u{25C9}",  // fisheye (double)
        }
    }
}

impl DamageType {
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Acid => "droplets",
            Self::Bludgeoning => "gavel",
            Self::Cold => "snowflake",
            Self::Fire => "flame",
            Self::Force => "sparkles",
            Self::Lightning => "zap",
            Self::Necrotic => "skull",
            Self::Piercing => "bow-arrow",
            Self::Poison => "flask-round",
            Self::Psychic => "brain",
            Self::Radiant => "sun",
            Self::Slashing => "sword",
            Self::Thunder => "cloud-lightning",
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, EnumIter, Display
)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Display, EnumIter)]
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
            Self::Strength => "ability-strength",
            Self::Dexterity => "ability-dexterity",
            Self::Constitution => "ability-constitution",
            Self::Intelligence => "ability-intelligence",
            Self::Wisdom => "ability-wisdom",
            Self::Charisma => "ability-charisma",
        }
    }

    fn tr_abbr_key(&self) -> &'static str {
        match self {
            Self::Strength => "ability-str",
            Self::Dexterity => "ability-dex",
            Self::Constitution => "ability-con",
            Self::Intelligence => "ability-int",
            Self::Wisdom => "ability-wis",
            Self::Charisma => "ability-cha",
        }
    }
}

impl Translatable for Skill {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::Acrobatics => "skill-acrobatics",
            Self::AnimalHandling => "skill-animal-handling",
            Self::Arcana => "skill-arcana",
            Self::Athletics => "skill-athletics",
            Self::Deception => "skill-deception",
            Self::History => "skill-history",
            Self::Insight => "skill-insight",
            Self::Intimidation => "skill-intimidation",
            Self::Investigation => "skill-investigation",
            Self::Medicine => "skill-medicine",
            Self::Nature => "skill-nature",
            Self::Perception => "skill-perception",
            Self::Performance => "skill-performance",
            Self::Persuasion => "skill-persuasion",
            Self::Religion => "skill-religion",
            Self::SleightOfHand => "skill-sleight-of-hand",
            Self::Stealth => "skill-stealth",
            Self::Survival => "skill-survival",
        }
    }
}

impl Translatable for Alignment {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::LawfulGood => "alignment-lawful-good",
            Self::NeutralGood => "alignment-neutral-good",
            Self::ChaoticGood => "alignment-chaotic-good",
            Self::LawfulNeutral => "alignment-lawful-neutral",
            Self::TrueNeutral => "alignment-true-neutral",
            Self::ChaoticNeutral => "alignment-chaotic-neutral",
            Self::LawfulEvil => "alignment-lawful-evil",
            Self::NeutralEvil => "alignment-neutral-evil",
            Self::ChaoticEvil => "alignment-chaotic-evil",
        }
    }
}

impl Translatable for Proficiency {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::LightArmor => "prof-light-armor",
            Self::MediumArmor => "prof-medium-armor",
            Self::HeavyArmor => "prof-heavy-armor",
            Self::Shields => "prof-shields",
            Self::SimpleWeapons => "prof-simple-weapons",
            Self::MartialWeapons => "prof-martial-weapons",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum SpellSlotPool {
    #[default]
    Arcane = 0,
    Pact = 1,
}
enum_serde_u8!(SpellSlotPool { Arcane, Pact });

impl SpellSlotPool {
    pub fn restore_on_short_rest(&self) -> bool {
        matches!(self, Self::Pact)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Display, Default)]
#[repr(u8)]
pub enum ArmorType {
    #[default]
    Light,
    Medium,
    Heavy,
    Shield,
    Natural,
}
enum_serde_u8!(ArmorType {
    Light,
    Medium,
    Heavy,
    Shield,
    Natural
});

impl ArmorType {
    /// Returns the `Proficiency` required to use this armor type,
    /// or `None` for Natural armor (no proficiency needed).
    pub fn required_proficiency(self) -> Option<Proficiency> {
        match self {
            Self::Light => Some(Proficiency::LightArmor),
            Self::Medium => Some(Proficiency::MediumArmor),
            Self::Heavy => Some(Proficiency::HeavyArmor),
            Self::Shield => Some(Proficiency::Shields),
            Self::Natural => None,
        }
    }
}

impl Translatable for DamageType {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::Acid => "damage-acid",
            Self::Bludgeoning => "damage-bludgeoning",
            Self::Cold => "damage-cold",
            Self::Fire => "damage-fire",
            Self::Force => "damage-force",
            Self::Lightning => "damage-lightning",
            Self::Necrotic => "damage-necrotic",
            Self::Piercing => "damage-piercing",
            Self::Poison => "damage-poison",
            Self::Psychic => "damage-psychic",
            Self::Radiant => "damage-radiant",
            Self::Slashing => "damage-slashing",
            Self::Thunder => "damage-thunder",
        }
    }
}

impl Translatable for SpellSlotPool {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::Arcane => "pool-arcane",
            Self::Pact => "pool-pact",
        }
    }
}

impl Translatable for ArmorType {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::Light => "armor-type-light",
            Self::Medium => "armor-type-medium",
            Self::Heavy => "armor-type-heavy",
            Self::Shield => "armor-type-shield",
            Self::Natural => "armor-type-natural",
        }
    }
}

#[cfg(test)]
pub mod tests {
    use wasm_bindgen_test::*;

    use super::*;

    #[wasm_bindgen_test]
    fn proficiency_level_multiplier() {
        assert_eq!(ProficiencyLevel::None.multiplier(), 0);
        assert_eq!(ProficiencyLevel::Proficient.multiplier(), 1);
        assert_eq!(ProficiencyLevel::Expertise.multiplier(), 2);
    }

    #[wasm_bindgen_test]
    fn proficiency_level_next_cycles() {
        assert_eq!(ProficiencyLevel::None.next(), ProficiencyLevel::Proficient);
        assert_eq!(
            ProficiencyLevel::Proficient.next(),
            ProficiencyLevel::Expertise
        );
        assert_eq!(ProficiencyLevel::Expertise.next(), ProficiencyLevel::None);
    }

    #[wasm_bindgen_test]
    fn proficiency_level_symbol() {
        assert_eq!(ProficiencyLevel::None.symbol(), "\u{25CB}");
        assert_eq!(ProficiencyLevel::Proficient.symbol(), "\u{25CF}");
        assert_eq!(ProficiencyLevel::Expertise.symbol(), "\u{25C9}");
    }

    #[wasm_bindgen_test]
    fn skill_ability_mapping() {
        assert_eq!(Skill::Athletics.ability(), Ability::Strength);

        assert_eq!(Skill::Acrobatics.ability(), Ability::Dexterity);
        assert_eq!(Skill::SleightOfHand.ability(), Ability::Dexterity);
        assert_eq!(Skill::Stealth.ability(), Ability::Dexterity);

        assert_eq!(Skill::Arcana.ability(), Ability::Intelligence);
        assert_eq!(Skill::History.ability(), Ability::Intelligence);
        assert_eq!(Skill::Investigation.ability(), Ability::Intelligence);
        assert_eq!(Skill::Nature.ability(), Ability::Intelligence);
        assert_eq!(Skill::Religion.ability(), Ability::Intelligence);

        assert_eq!(Skill::AnimalHandling.ability(), Ability::Wisdom);
        assert_eq!(Skill::Insight.ability(), Ability::Wisdom);
        assert_eq!(Skill::Medicine.ability(), Ability::Wisdom);
        assert_eq!(Skill::Perception.ability(), Ability::Wisdom);
        assert_eq!(Skill::Survival.ability(), Ability::Wisdom);

        assert_eq!(Skill::Deception.ability(), Ability::Charisma);
        assert_eq!(Skill::Intimidation.ability(), Ability::Charisma);
        assert_eq!(Skill::Performance.ability(), Ability::Charisma);
        assert_eq!(Skill::Persuasion.ability(), Ability::Charisma);
    }

    #[wasm_bindgen_test]
    fn ability_serde_u8_roundtrip() {
        let ability = Ability::Wisdom;
        let json = serde_json::to_string(&ability).unwrap();
        assert_eq!(json, "4"); // Wisdom is index 4
        let deserialized: Ability = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ability);
    }

    #[wasm_bindgen_test]
    fn ability_serde_all_variants() {
        use strum::IntoEnumIterator;
        for (i, ability) in Ability::iter().enumerate() {
            let json = serde_json::to_string(&ability).unwrap();
            assert_eq!(json, i.to_string());
            let back: Ability = serde_json::from_str(&json).unwrap();
            assert_eq!(back, ability);
        }
    }

    #[wasm_bindgen_test]
    fn skill_serde_u8_roundtrip() {
        let skill = Skill::Stealth;
        let json = serde_json::to_string(&skill).unwrap();
        let deserialized: Skill = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, skill);
    }

    #[wasm_bindgen_test]
    fn proficiency_level_serde_u8_roundtrip() {
        let pl = ProficiencyLevel::Expertise;
        let json = serde_json::to_string(&pl).unwrap();
        assert_eq!(json, "2");
        let deserialized: ProficiencyLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, pl);
    }
}
