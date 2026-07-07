use std::collections::BTreeMap;

use serde::Deserialize;
use strum::{Display, EnumIter, EnumString, VariantArray};

use crate::{
    demap::{self, Named},
    model::{ActionType, EffectDefinition, EffectDuration, EffectRange, Money, Translatable},
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
    #[serde(default)]
    pub components: SpellComponents,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SpellComponents {
    #[serde(default)]
    pub verbal: bool,
    #[serde(default)]
    pub somatic: bool,
    #[serde(default)]
    pub material: Option<MaterialComponent>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MaterialComponent {
    #[serde(default)]
    pub consumable: bool,
    #[serde(default)]
    pub name: Box<str>,
    #[serde(default)]
    pub price: Option<Money>,
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
    /// Host spellcasting feature whose learnable list this feature's `list`
    /// extends. Extenders get no spellcasting block of their own.
    #[serde(default)]
    pub extends: Option<String>,
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
            "(SLOT.LEVEL * 2)d4",
            "(SLOT.LEVEL + 2)d6",
            "(SLOT.LEVEL)d8",
            "(SLOT.LEVEL / 2)d8 + CASTER.MOD",
            "SLOT.LEVEL / 2",
            "if(LEVEL >= 17, 4, if(LEVEL >= 11, 3, if(LEVEL >= 5, 2, 1)))d6",
            // These use implicit dice after a bare variable (space before d)
            "SLOT.LEVEL d6",
            "SLOT.LEVEL d4 + CASTER.MOD",
            "2d8 + SLOT.LEVEL d6",
            "SLOT.LEVEL / 2 d8 + CASTER.MOD",
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
    fn spells_definition_extends_deserializes() {
        let extender: SpellsDefinition = serde_json::from_value(serde_json::json!({
            "extends": "Pact Magic",
            "list": [{"name": "Sanctuary"}],
        }))
        .expect("extender block");
        assert_eq!(extender.extends.as_deref(), Some("Pact Magic"));

        let plain: SpellsDefinition = serde_json::from_value(serde_json::json!({
            "list": [{"name": "Magic Missile"}],
        }))
        .expect("plain block");
        assert!(plain.extends.is_none());
    }

    #[test]
    fn spell_with_full_components_deserializes() {
        let json = serde_json::json!({
            "name": "Test Spell",
            "components": {
                "verbal": true,
                "somatic": false,
                "material": { "consumable": true, "name": "a pinch of sulfur" }
            }
        });
        let def: SpellDefinition = serde_json::from_value(json).expect("must deserialize");
        assert!(def.components.verbal);
        assert!(!def.components.somatic);
        let material = def.components.material.expect("material present");
        assert!(material.consumable);
        assert_eq!(&*material.name, "a pinch of sulfur");
    }

    #[test]
    fn spell_without_components_defaults_to_empty() {
        let json = serde_json::json!({ "name": "Bare Spell" });
        let def: SpellDefinition = serde_json::from_value(json).expect("must deserialize");
        assert!(!def.components.verbal);
        assert!(!def.components.somatic);
        assert!(def.components.material.is_none());
    }

    #[test]
    fn material_price_deserializes_as_copper() {
        let json = serde_json::json!({
            "name": "Test Spell",
            "components": { "material": { "name": "diamonds", "price": 30000 } }
        });
        let def: SpellDefinition = serde_json::from_value(json).expect("must deserialize");
        let material = def.components.material.expect("material present");
        assert_eq!(&*material.name, "diamonds");
        assert_eq!(material.price, Some(Money::from_gp(300)));
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn apply_skips_spell_data_for_extenders() {
        use crate::{
            model::{Character, Feature},
            rules::{FeatureDefinition, WhenCondition, apply::apply_feature},
        };

        let plain: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Pact Magic",
            "spells": {"list": [{"name": "Magic Missile"}]},
        }))
        .expect("plain def");
        let extender: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Expanded Spell List (Dao)",
            "spells": {"extends": "Pact Magic", "list": [{"name": "Sanctuary"}]},
        }))
        .expect("extender def");

        let mut character = Character::default();
        for def in [&plain, &extender] {
            let pos = character.features.push(Feature {
                name: def.name.clone(),
                applied: true,
                ..Feature::default()
            });
            apply_feature(def, &mut character, pos, WhenCondition::OnFeatureAdd);
        }

        assert!(
            character
                .features
                .data()
                .get("Pact Magic")
                .is_some_and(|entry| entry.spells.is_some()),
            "plain spell feature gets its SpellData block"
        );
        assert!(
            character
                .features
                .data()
                .get("Expanded Spell List (Dao)")
                .is_none_or(|entry| entry.spells.is_none()),
            "extender must not get a spellcasting block of its own"
        );
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
