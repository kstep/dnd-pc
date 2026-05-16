use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use strum::VariantArray;

use crate::{
    expr::{ResolveGroup, VarGroup},
    model::{
        Ability, Attribute, CharacterCore, DamageType, Skill,
        attribute::{intern, parse_ability, parse_damage_type, parse_skill},
    },
};

// Per-group ZST descriptors. Each holds a constructor table mapping
// `Index` to a concrete `Attribute`, plus a schema indexed by `col_no`
// (`schema()[0] == "ARG"`, `schema()[1] == ""` = primary, the rest =
// suffixes parallel to `columns()`).

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityGroup;

impl VarGroup for AbilityGroup {
    type Index = Ability;
    type Var = Attribute;

    const COLUMNS: &'static [fn(Ability) -> Attribute] = &[
        Attribute::Ability,
        Attribute::Modifier,
        Attribute::SavingThrow,
        Attribute::SaveProficiency,
        Attribute::SaveAdvantage,
        Attribute::AbilityAdvantage,
    ];
    const SCHEMA: &'static [&'static str] = &["MOD", "SAVE", "SAVE.PROF", "SAVE.ADV", "ADV"];

    fn arg(n: u8) -> Attribute {
        Attribute::Arg(n)
    }

    fn member_by_name(&self, name: &str) -> Option<usize> {
        let ability = parse_ability(name)?;
        Ability::VARIANTS.iter().position(|&x| x == ability)
    }

    fn member_name(&self, pos: usize) -> Option<&'static str> {
        Ability::VARIANTS.get(pos).map(|ability| ability.abbr())
    }

    fn static_size(&self) -> Option<usize> {
        Some(Ability::VARIANTS.len())
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillGroup;

impl VarGroup for SkillGroup {
    type Index = Skill;
    type Var = Attribute;

    const COLUMNS: &'static [fn(Skill) -> Attribute] = &[
        Attribute::Skill,
        Attribute::SkillProficiency,
        Attribute::SkillAdvantage,
    ];
    const SCHEMA: &'static [&'static str] = &["PROF", "ADV"];

    fn arg(n: u8) -> Attribute {
        Attribute::Arg(n)
    }

    fn member_by_name(&self, name: &str) -> Option<usize> {
        let skill = parse_skill(name)?;
        Skill::VARIANTS.iter().position(|&x| x == skill)
    }

    fn member_name(&self, pos: usize) -> Option<&'static str> {
        Skill::VARIANTS.get(pos).map(|skill| skill.abbr())
    }

    fn static_size(&self) -> Option<usize> {
        Some(Skill::VARIANTS.len())
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolGroup;

impl VarGroup for ToolGroup {
    type Index = &'static str;
    type Var = Attribute;

    const COLUMNS: &'static [fn(&'static str) -> Attribute] =
        &[Attribute::ToolProficiency, Attribute::ToolAbility];
    const SCHEMA: &'static [&'static str] = &["ABIL"];

    fn arg(n: u8) -> Attribute {
        Attribute::Arg(n)
    }

    fn member_by_name(&self, _name: &str) -> Option<usize> {
        // Positional masks unsupported — tool positions depend on
        // per-character `Character.tools` order. Name masks are used
        // instead via `SubgroupMask::Names` + `name_of`.
        None
    }

    fn name_of(&self, row: &[Attribute]) -> Option<&'static str> {
        match row.get(1)? {
            Attribute::ToolProficiency(name) => Some(name),
            _ => None,
        }
    }

    fn index_from_name(&self, name: &'static str) -> Option<&'static str> {
        Some(name)
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmgGroup;

impl VarGroup for DmgGroup {
    type Index = DamageType;
    type Var = Attribute;

    const COLUMNS: &'static [fn(DamageType) -> Attribute] = &[
        Attribute::Resistance,
        Attribute::Vulnerability,
        Attribute::Immunity,
        Attribute::DamageReduction,
    ];
    const SCHEMA: &'static [&'static str] = &["VULN", "IMMUNE", "DR"];

    fn arg(n: u8) -> Attribute {
        Attribute::Arg(n)
    }

    fn member_by_name(&self, name: &str) -> Option<usize> {
        let damage = parse_damage_type(name)?;
        DamageType::VARIANTS.iter().position(|&x| x == damage)
    }

    fn member_name(&self, pos: usize) -> Option<&'static str> {
        DamageType::VARIANTS.get(pos).map(|damage| damage.abbr())
    }

    fn static_size(&self) -> Option<usize> {
        Some(DamageType::VARIANTS.len())
    }
}

/// Storage discriminator for Op-side dispatch.
///
/// Parser builds it via `FromStr` from bare group names (`ABIL`,
/// `SKILL`, `TOOL`, `DMG`). Iteration is handled by
/// `Context::resolve_group` (per-variant impl on the concrete Context),
/// not through `VarGroup` methods here — those provide
/// schema/member_by_name for parser-side use.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeGroup {
    #[default]
    Ability,
    Skill,
    Tool,
    Dmg,
}

impl FromStr for AttributeGroup {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ABIL" => Ok(Self::Ability),
            "SKILL" => Ok(Self::Skill),
            "TOOL" => Ok(Self::Tool),
            "DMG" => Ok(Self::Dmg),
            _ => Err("unknown attribute group"),
        }
    }
}

impl AttributeGroup {
    /// Materialise rows that don't depend on Character state: Ability,
    /// Skill, Dmg variants enumerate their `VARIANTS`. Tool returns
    /// empty (its membership is per-character).
    pub fn static_rows(&self) -> Box<dyn Iterator<Item = Vec<Attribute>>> {
        match self {
            Self::Ability => Box::new(
                Ability::VARIANTS
                    .iter()
                    .copied()
                    .enumerate()
                    .map(AbilityGroup::make_row),
            ),
            Self::Skill => Box::new(
                Skill::VARIANTS
                    .iter()
                    .copied()
                    .enumerate()
                    .map(SkillGroup::make_row),
            ),
            Self::Tool => Box::new(std::iter::empty()),
            Self::Dmg => Box::new(
                DamageType::VARIANTS
                    .iter()
                    .copied()
                    .enumerate()
                    .map(DmgGroup::make_row),
            ),
        }
    }
}

impl CharacterCore {
    pub fn resolve_attribute_group<'a>(
        &'a self,
        grp: &AttributeGroup,
    ) -> Box<dyn Iterator<Item = Vec<Attribute>> + 'a> {
        match grp {
            AttributeGroup::Tool => Box::new(
                self.tools
                    .iter()
                    .enumerate()
                    .map(|(i, entry)| ToolGroup::make_row((i, intern(&entry.name)))),
            ),
            other => other.static_rows(),
        }
    }
}

impl<T: AsRef<CharacterCore> + ?Sized> ResolveGroup<AttributeGroup> for T {
    fn resolve_group<'a>(
        &'a self,
        grp: &AttributeGroup,
    ) -> Box<dyn Iterator<Item = Vec<Attribute>> + 'a> {
        self.as_ref().resolve_attribute_group(grp)
    }
}

/// Context-free source for static attribute-group rows. Used by
/// analysis-only passes (AI prompt builder, reference renderer, form
/// builder) that walk Op-streams without a Character available. Tool
/// iteration yields nothing here.
#[derive(Debug, Copy, Clone, Default)]
pub struct StaticAttrSource;

impl ResolveGroup<AttributeGroup> for StaticAttrSource {
    fn resolve_group<'a>(
        &'a self,
        grp: &AttributeGroup,
    ) -> Box<dyn Iterator<Item = Vec<Attribute>> + 'a> {
        grp.static_rows()
    }
}

impl fmt::Display for AttributeGroup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Self::Ability => "ABIL",
            Self::Skill => "SKILL",
            Self::Tool => "TOOL",
            Self::Dmg => "DMG",
        })
    }
}

/// Shim `VarGroup` impl on the storage enum: only `column_by_suffix` and
/// `member_by_name` are reached (by parser). Row iteration goes through
/// `ResolveGroup::resolve_group` on the Context side.
impl VarGroup for AttributeGroup {
    type Index = ();
    type Var = Attribute;

    const COLUMNS: &'static [fn(()) -> Attribute] = &[];
    const SCHEMA: &'static [&'static str] = &[];

    fn arg(n: u8) -> Attribute {
        Attribute::Arg(n)
    }

    fn column_by_suffix(&self, suffix: &str) -> Option<u8> {
        match self {
            Self::Ability => AbilityGroup.column_by_suffix(suffix),
            Self::Skill => SkillGroup.column_by_suffix(suffix),
            Self::Tool => ToolGroup.column_by_suffix(suffix),
            Self::Dmg => DmgGroup.column_by_suffix(suffix),
        }
    }

    fn member_by_name(&self, name: &str) -> Option<usize> {
        match self {
            Self::Ability => AbilityGroup.member_by_name(name),
            Self::Skill => SkillGroup.member_by_name(name),
            Self::Tool => ToolGroup.member_by_name(name),
            Self::Dmg => DmgGroup.member_by_name(name),
        }
    }

    fn member_name(&self, pos: usize) -> Option<&'static str> {
        match self {
            Self::Ability => AbilityGroup.member_name(pos),
            Self::Skill => SkillGroup.member_name(pos),
            Self::Tool => ToolGroup.member_name(pos),
            Self::Dmg => DmgGroup.member_name(pos),
        }
    }

    fn name_of(&self, row: &[Attribute]) -> Option<&'static str> {
        match self {
            Self::Tool => ToolGroup.name_of(row),
            Self::Ability | Self::Skill | Self::Dmg => None,
        }
    }

    fn materialize_from_name(&self, name: &'static str, iter_no: usize) -> Option<Vec<Attribute>> {
        match self {
            Self::Tool => Some(ToolGroup::make_row((iter_no, name))),
            Self::Ability | Self::Skill | Self::Dmg => None,
        }
    }

    fn static_size(&self) -> Option<usize> {
        match self {
            Self::Ability => AbilityGroup.static_size(),
            Self::Skill => SkillGroup.static_size(),
            Self::Tool => ToolGroup.static_size(),
            Self::Dmg => DmgGroup.static_size(),
        }
    }
}
