use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    EnumIter,
    Display
)]
pub enum Ability {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    EnumIter,
    Display
)]
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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Display,
    EnumIter
)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProficiencyLevel {
    None,
    Proficient,
    Expertise,
}

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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    EnumIter,
    Display
)]
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

#[allow(dead_code)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Display,
    EnumIter
)]
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
