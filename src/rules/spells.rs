use std::collections::BTreeMap;

use serde::Deserialize;
use strum::{Display, EnumIter, EnumString, VariantArray};

use crate::{
    demap::{self, Named},
    model::{
        Character, EffectDefinition, EffectDuration, EffectRange, FreeUses, Spell, SpellData,
        Translatable,
    },
    rules::feature::{ActionType, FeatureDefinition},
};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
pub enum CastTime {
    Action(ActionType),
    Rounds(u32),
}

impl Default for CastTime {
    fn default() -> Self {
        CastTime::Action(ActionType::Action)
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    Deserialize,
    Display,
    EnumIter,
    EnumString,
    VariantArray
)]
#[repr(u8)]
pub enum SpellCategory {
    Damage,
    Healing,
    Buff,
    Debuff,
    Control,
    Defense,
    #[default]
    Utility,
    Summon,
    Social,
}

impl Translatable for SpellCategory {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::Damage => "spell-cat-damage",
            Self::Healing => "spell-cat-healing",
            Self::Buff => "spell-cat-buff",
            Self::Debuff => "spell-cat-debuff",
            Self::Control => "spell-cat-control",
            Self::Defense => "spell-cat-defense",
            Self::Utility => "spell-cat-utility",
            Self::Summon => "spell-cat-summon",
            Self::Social => "spell-cat-social",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpellDefinition {
    pub name: Box<str>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub ritual: bool,
    #[serde(default)]
    pub concentration: bool,
    #[serde(default)]
    pub cast_time: CastTime,
    #[serde(default)]
    pub category: SpellCategory,
    #[serde(default)]
    pub effects: Vec<EffectDefinition>,
}

#[derive(Clone, Copy)]
pub struct SpellMeta {
    pub cast_time: CastTime,
    pub ritual: bool,
    pub concentration: bool,
    pub category: SpellCategory,
    pub range: Option<EffectRange>,
    pub duration: Option<EffectDuration>,
}

impl SpellDefinition {
    pub fn effect_range(&self) -> Option<EffectRange> {
        self.effects.iter().map(|effect| effect.range).max()
    }

    pub fn effect_duration(&self) -> Option<EffectDuration> {
        self.effects.iter().map(|effect| effect.duration).max()
    }

    pub fn meta(&self) -> SpellMeta {
        SpellMeta {
            cast_time: self.cast_time,
            ritual: self.ritual,
            concentration: self.concentration,
            category: self.category,
            range: self.effect_range(),
            duration: self.effect_duration(),
        }
    }
}

impl Named for SpellDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Global spells index — all spell definitions keyed by name. Mirrors
/// `FeaturesIndex`. Loaded once from `public/data/spells.json`.
#[derive(Clone, Default)]
pub struct SpellsIndex(pub BTreeMap<Box<str>, SpellDefinition>);

/// Empty fallback for callers that need a stable reference when the index
/// hasn't loaded yet (registry's `with_spells_index*`) or when running
/// dry-runs that don't care about sticky imports (solver, rebuild tests).
pub static EMPTY_SPELL_INDEX: BTreeMap<Box<str>, SpellDefinition> = BTreeMap::new();

impl<'de> Deserialize<'de> for SpellsIndex {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        demap::named_map(deserializer).map(Self)
    }
}

impl std::ops::Deref for SpellsIndex {
    type Target = BTreeMap<Box<str>, SpellDefinition>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Per-feature spell block. Either an explicit list of entries (with optional
/// per-class metadata) or a reference to a curated per-class name list.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SpellsDefinition {
    #[serde(default)]
    pub list: SpellsList,
    /// Name of a field (`Points` or `FreeUses`) backing the per-cast cost,
    /// looked up by name across the character's features — not necessarily on
    /// the same feature.
    #[serde(default)]
    pub cost: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SpellsList {
    Inline(Vec<SpellEntry>),
    Ref { from: String },
}

impl Default for SpellsList {
    fn default() -> Self {
        Self::Inline(Vec::new())
    }
}

impl SpellsList {
    /// Extract the short list name from a `Ref` path
    /// (`"spells/wizard.json"` → `"wizard"`).
    pub fn ref_name(&self) -> Option<&str> {
        match self {
            Self::Ref { from } => from
                .strip_prefix("spells/")
                .and_then(|rest| rest.strip_suffix(".json")),
            _ => None,
        }
    }

    /// Build a ref path from a short list name
    /// (`"wizard"` → `"spells/wizard.json"`).
    pub fn ref_path(name: &str) -> String {
        format!("spells/{name}.json")
    }

    /// Inline entries from a feature's spell block. Empty for `Ref` lists.
    pub fn inline_entries(&self) -> &[SpellEntry] {
        match self {
            Self::Inline(entries) => entries,
            Self::Ref { .. } => &[],
        }
    }
}

/// One spell entry inside a feature's inline list. JSON accepts both a bare
/// string (`"Magic Missile"`) and an object (`{"name": "...", "sticky": true,
/// "min_level": 3, "cost": 1}`); fields default to zero/false when omitted.
#[derive(Debug, Clone, Default)]
pub struct SpellEntry {
    pub name: Box<str>,
    pub sticky: bool,
    pub min_level: u32,
    pub cost: u32,
}

impl<'de> Deserialize<'de> for SpellEntry {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Bare(Box<str>),
            Detailed {
                name: Box<str>,
                #[serde(default)]
                sticky: bool,
                #[serde(default)]
                min_level: u32,
                #[serde(default)]
                cost: u32,
            },
        }
        match Repr::deserialize(deserializer)? {
            Repr::Bare(name) => Ok(SpellEntry {
                name,
                ..SpellEntry::default()
            }),
            Repr::Detailed {
                name,
                sticky,
                min_level,
                cost,
            } => Ok(SpellEntry {
                name,
                sticky,
                min_level,
                cost,
            }),
        }
    }
}

impl SpellsDefinition {
    /// Per-feature SpellData bootstrap: ensures `SpellData` exists, imports
    /// sticky entries (looking up their full definition in `spells_index`),
    /// and refreshes `free_uses.max` after a level-up. `Ref` blocks have no
    /// per-entry sticky info, so they only ensure the skeleton.
    pub fn apply(
        &self,
        feat_def: &FeatureDefinition,
        level: u32,
        character: &mut Character,
        spells_index: &BTreeMap<Box<str>, SpellDefinition>,
    ) {
        let feature_name: &str = &feat_def.name;
        let free_uses_max = feat_def.free_uses_max(level, character);
        let entry = character
            .features
            .entry(feature_name.to_string())
            .or_default();
        entry.spells.get_or_insert_with(SpellData::default);

        let Some(spell_data) = character.features.spell_data_mut(feature_name) else {
            return;
        };

        for entry in self
            .list
            .inline_entries()
            .iter()
            .filter(|entry| entry.sticky && entry.min_level <= level)
        {
            if spell_data
                .spells
                .iter()
                .any(|existing| existing.name.as_str() == &*entry.name)
            {
                continue;
            }
            let Some(def) = spells_index.get(&*entry.name) else {
                continue;
            };
            let free_uses = (entry.cost > 0 && free_uses_max > 0).then_some(FreeUses {
                used: 0,
                max: free_uses_max,
            });
            spell_data.spells.push(Spell {
                name: def.name.to_string(),
                label: None,
                description: String::new(),
                level: def.level,
                sticky: true,
                cost: entry.cost,
                free_uses,
            });
        }

        // Update free_uses.max on existing prepared spells (level-up).
        if free_uses_max > 0 {
            for spell in &mut spell_data.spells {
                if spell.cost > 0 {
                    spell
                        .free_uses
                        .get_or_insert(FreeUses {
                            used: 0,
                            max: free_uses_max,
                        })
                        .max = free_uses_max;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn parse_spells_index() -> SpellsIndex {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("public/data/spells.json");
        let data = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_str::<SpellsIndex>(&data)
            .unwrap_or_else(|error| panic!("failed to parse spells.json: {error}"))
    }

    #[test]
    fn parse_expr_with_mul_dice() {
        use crate::model::Expr;
        let cases = [
            "(SLOT_LEVEL * 2)d4",
            "(SLOT_LEVEL + 2)d6",
            "(SLOT_LEVEL)d8",
            "(SLOT_LEVEL / 2)d8 + CASTER_MODIFIER",
            "SLOT_LEVEL / 2",
            "if(LEVEL >= 17, 4, if(LEVEL >= 11, 3, if(LEVEL >= 5, 2, 1)))d6",
            // These use implicit dice after a bare variable (space before d)
            "SLOT_LEVEL d6",
            "SLOT_LEVEL d4 + CASTER_MODIFIER",
            "2d8 + SLOT_LEVEL d6",
            "SLOT_LEVEL / 2 d8 + CASTER_MODIFIER",
        ];
        for expr_str in cases {
            let result = expr_str.parse::<Expr>();
            assert!(
                result.is_ok(),
                "failed to parse '{expr_str}': {:?}",
                result.err()
            );
        }
    }

    #[test]
    fn deserialize_spells_json() {
        let index = parse_spells_index();
        assert!(
            index.0.len() > 500,
            "expected 500+ spells in index, got {}",
            index.0.len()
        );
    }

    #[test]
    fn deserialize_spells_json_has_categories() {
        let index = parse_spells_index();
        let fireball = index.get("Fireball").expect("Fireball present");
        assert_eq!(fireball.category, SpellCategory::Damage);
        let cure_wounds = index.get("Cure Wounds").expect("Cure Wounds present");
        assert_eq!(cure_wounds.category, SpellCategory::Healing);
    }

    #[test]
    fn no_unintended_utility_default() {
        let index = parse_spells_index();
        let utility = index
            .values()
            .filter(|sp| sp.category == SpellCategory::Utility)
            .count();
        assert!(
            utility <= 170,
            "Too many spells defaulted to Utility ({utility}); markup likely broken"
        );
    }

    #[test]
    fn per_class_spell_lists_resolve_into_index() {
        let index = parse_spells_index();
        let lists_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("public/data/spells");
        let classes = [
            "artificer",
            "bard",
            "cleric",
            "druid",
            "paladin",
            "ranger",
            "sorcerer",
            "warlock",
            "wizard",
        ];
        for name in classes {
            let path = lists_dir.join(format!("{name}.json"));
            let data = std::fs::read_to_string(&path).expect("read class list");
            let names: Vec<String> =
                serde_json::from_str(&data).expect("class list is array of names");
            assert!(!names.is_empty(), "{name}.json should have spell names");
            for spell_name in &names {
                assert!(
                    index.0.contains_key(spell_name.as_str()),
                    "{name}.json references unknown spell {spell_name:?}"
                );
            }
        }
    }

    #[test]
    fn all_spell_effects_have_valid_expressions() {
        let index = parse_spells_index();
        let mut total_effects = 0;
        for (spell_name, spell) in index.0.iter() {
            for effect in &spell.effects {
                total_effects += 1;
                if let Some(ref expr) = effect.expr {
                    let display = expr.to_string();
                    assert!(
                        !display.is_empty(),
                        "{spell_name}: effect '{}' has empty expression display",
                        effect.name
                    );
                }
            }
        }
        assert!(
            total_effects > 100,
            "expected 100+ spell effects in index, got {total_effects}"
        );
    }
}
