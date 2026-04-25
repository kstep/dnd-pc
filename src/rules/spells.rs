use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    demap::{self, Named},
    model::{Character, EffectDefinition, EffectDuration, EffectRange, FreeUses, Spell, SpellData},
    rules::feature::ActionType,
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

#[derive(Debug, Clone, Deserialize)]
pub struct SpellDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub sticky: bool,
    #[serde(default)]
    pub min_level: u32,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub ritual: bool,
    #[serde(default)]
    pub concentration: bool,
    #[serde(default)]
    pub cast_time: CastTime,
    #[serde(default)]
    pub effects: Vec<EffectDefinition>,
}

#[derive(Clone, Copy)]
pub struct SpellMeta {
    pub cast_time: CastTime,
    pub ritual: bool,
    pub concentration: bool,
    pub range: Option<EffectRange>,
    pub duration: Option<EffectDuration>,
}

impl SpellDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn effect_range(&self) -> Option<EffectRange> {
        self.effects.iter().map(|e| e.range).max()
    }

    pub fn effect_duration(&self) -> Option<EffectDuration> {
        self.effects.iter().map(|e| e.duration).max()
    }

    pub fn meta(&self) -> SpellMeta {
        SpellMeta {
            cast_time: self.cast_time,
            ritual: self.ritual,
            concentration: self.concentration,
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SpellsDefinition {
    #[serde(default)]
    pub list: SpellList,
    #[serde(default)]
    pub cost: Option<String>,
}

impl SpellsDefinition {
    /// Bootstrap per-feature `SpellData`: ensure the entry exists, import
    /// sticky spells from an inline list, and refresh `free_uses.max` on
    /// existing prepared spells. Numeric scaling (slot totals, prepared /
    /// known counts, cantrips) is now driven by `OnFeatureAdd` /
    /// `OnCompute` `assign` expressions on the feature itself — see
    /// `Slot`, `SlotPool`, `CasterAbility`, `CasterCoef`,
    /// `SpellCantrips`, `SpellReady`, `SpellKnown` resolvers in
    /// `src/model/character.rs`.
    pub fn apply(
        &self,
        level: u32,
        character: &mut Character,
        feature_name: &str,
        free_uses_max: u32,
    ) {
        // Skeleton — ensure SpellData exists so subsequent OnFeatureAdd
        // assigns (`SLOT.POOL`, `CASTER_ABILITY`, `CASTER_COEF`) have a
        // target. Persisted SpellData on load is already the source of
        // truth for pool / ability / coef; OnFeatureAdd writes them only
        // on the first add.
        let entry = character
            .features
            .entry(feature_name.to_string())
            .or_default();
        entry.spells.get_or_insert_with(SpellData::default);

        let Some(spell_data) = character.features.spell_data_mut(feature_name) else {
            return;
        };

        // Sticky spells from inline list — route to known (spellbook) if
        // two-tier.
        if let SpellList::Inline(list) = &self.list {
            let two_tier = spell_data.is_two_tier();
            let target = if two_tier {
                spell_data.known.get_or_insert_with(Vec::new)
            } else {
                &mut spell_data.spells
            };
            for source in list.values().filter(|s| s.sticky && s.min_level <= level) {
                if target.iter().any(|existing| existing.name == source.name) {
                    continue;
                }
                let free_uses =
                    (!two_tier && source.cost > 0 && free_uses_max > 0).then_some(FreeUses {
                        used: 0,
                        max: free_uses_max,
                    });
                target.push(Spell {
                    name: source.name.clone(),
                    label: source.label.clone(),
                    description: source.description.clone(),
                    level: source.level,
                    sticky: true,
                    cost: source.cost,
                    free_uses,
                });
            }
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

/// A map of spell definitions keyed by name. Deserializes from a JSON array
/// `[{"name": ...}, ...]` into `BTreeMap<Box<str>, SpellDefinition>` via
/// `named_map`.
#[derive(Debug, Clone, Default)]
pub struct SpellMap(pub BTreeMap<Box<str>, SpellDefinition>);

impl<'de> Deserialize<'de> for SpellMap {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        demap::named_map(deserializer).map(Self)
    }
}

impl std::ops::Deref for SpellMap {
    type Target = BTreeMap<Box<str>, SpellDefinition>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SpellList {
    Ref { from: String },
    Inline(SpellMap),
}

impl SpellList {
    /// Extract the short list name from a `Ref` path (e.g.
    /// `"spells/wizard.json"` → `"wizard"`).
    pub fn ref_name(&self) -> Option<&str> {
        match self {
            Self::Ref { from } => from
                .strip_prefix("spells/")
                .and_then(|s| s.strip_suffix(".json")),
            _ => None,
        }
    }

    /// Build a ref path from a short list name (e.g. `"wizard"` →
    /// `"spells/wizard.json"`).
    pub fn ref_path(name: &str) -> String {
        format!("spells/{name}.json")
    }
}

impl Default for SpellList {
    fn default() -> Self {
        Self::Inline(SpellMap::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_spell_list(name: &str) -> SpellMap {
        let path = format!("../../public/data/spells/{name}.json");
        let data = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("public/data/spells")
                .join(format!("{name}.json")),
        )
        .unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
        serde_json::from_str::<SpellMap>(&data)
            .unwrap_or_else(|e| panic!("failed to parse {path}: {e}"))
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
    fn deserialize_all_spell_lists() {
        let lists = [
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
        for name in lists {
            let map = parse_spell_list(name);
            assert!(!map.0.is_empty(), "{name}.json should have spells");
        }
    }

    #[test]
    fn all_spell_effects_have_valid_expressions() {
        let lists = [
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
        let mut total_effects = 0;
        for name in lists {
            let map = parse_spell_list(name);
            for (spell_name, spell) in map.0.iter() {
                for effect in &spell.effects {
                    total_effects += 1;
                    if let Some(ref expr) = effect.expr {
                        // Verify the expression can be displayed (round-trip check)
                        let display = format!("{}", expr);
                        assert!(
                            !display.is_empty(),
                            "{name}/{spell_name}: effect '{}' has empty expression display",
                            effect.name
                        );
                    }
                }
            }
        }
        assert!(
            total_effects > 100,
            "expected 100+ spell effects across all lists, got {total_effects}"
        );
    }
}
