use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    demap::{self, Named},
    model::{Ability, Character, FeatureSource, FreeUses, Spell, SpellData, SpellSlotPool},
};

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
}

impl SpellDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

impl Named for SpellDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpellsDefinition {
    pub casting_ability: Ability,
    #[serde(default)]
    pub caster_coef: u32,
    #[serde(default)]
    pub pool: SpellSlotPool,
    #[serde(default)]
    pub list: SpellList,
    #[serde(default)]
    pub levels: Vec<SpellLevelRules>,
    #[serde(default)]
    pub cost: Option<String>,
}

impl SpellsDefinition {
    /// Ensure SpellData exists on the feature_data entry and update spell
    /// slots.
    pub fn apply(
        &self,
        level: u32,
        character: &mut Character,
        feature_name: &str,
        source: &FeatureSource,
        free_uses_max: u32,
    ) {
        // Ensure SpellData exists so update_spell_slots can count caster classes
        {
            let entry = character
                .feature_data
                .entry(feature_name.to_string())
                .or_default();
            if entry.source.is_none() {
                entry.source = Some(source.clone());
            }
            entry.spells.get_or_insert_with(|| SpellData {
                casting_ability: self.casting_ability,
                caster_coef: self.caster_coef,
                pool: self.pool,
                ..Default::default()
            });
        }

        // Update spell slots (needs separate borrow scope)
        let slots = self
            .levels
            .get(level as usize - 1)
            .and_then(|level_rules| level_rules.slots.as_deref());
        character.update_spell_slots(self.pool, slots);

        let highest_slot_level = slots
            .and_then(|s| s.iter().rposition(|&n| n > 0))
            .map_or(1, |i| (i + 1) as u32);

        let Some(entry) = character.feature_data.get_mut(feature_name) else {
            return;
        };
        let Some(spell_data) = entry.spells.as_mut() else {
            return;
        };

        // Add new cantrip/spell slots based on level rules
        if let Some(rules) = self.levels.get(level as usize - 1) {
            let (cantrips_current, spells_current) = spell_data
                .spells
                .iter()
                .filter(|s| !s.sticky)
                .fold((0usize, 0usize), |(cantrips, spells), spell| {
                    if spell.level == 0 {
                        (cantrips + 1, spells)
                    } else {
                        (cantrips, spells + 1)
                    }
                });

            let cantrips_target = rules.cantrips.unwrap_or(cantrips_current as u32) as usize;
            let spells_target = rules.spells.unwrap_or(spells_current as u32) as usize;

            spell_data.spells.extend(
                (cantrips_current..cantrips_target)
                    .map(|_| Spell {
                        level: 0,
                        ..Default::default()
                    })
                    .chain((spells_current..spells_target).map(|_| Spell {
                        level: highest_slot_level,
                        ..Default::default()
                    })),
            );
        }

        // Sticky spells from inline list
        if let SpellList::Inline(list) = &self.list {
            for s in list.values().filter(|s| s.sticky && s.min_level <= level) {
                if !spell_data.spells.iter().any(|ex| ex.name == s.name) {
                    spell_data.spells.push(Spell {
                        name: s.name.clone(),
                        label: s.label.clone(),
                        description: s.description.clone(),
                        level: s.level,
                        prepared: true,
                        sticky: true,
                        cost: s.cost,
                        free_uses: (s.cost > 0 && free_uses_max > 0).then_some(FreeUses {
                            used: 0,
                            max: free_uses_max,
                        }),
                    });
                }
            }
        }

        // Update free_uses.max on existing spells (level-up)
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SpellLevelRules {
    #[serde(default)]
    pub cantrips: Option<u32>,
    #[serde(default)]
    pub spells: Option<u32>,
    #[serde(default)]
    pub slots: Option<Vec<u32>>,
}
