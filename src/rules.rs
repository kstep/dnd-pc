use std::collections::{BTreeMap, HashMap, btree_map::Entry};

use leptos::prelude::*;
use serde::Deserialize;

use crate::{
    BASE_URL,
    model::{Ability, Character, Feature, FeatureField, FeatureValue, Proficiency, Spell},
};

// --- JSON types ---

#[derive(Debug, Clone, Deserialize)]
struct Index {
    classes: Vec<ClassIndexEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassIndexEntry {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub prerequisites: Vec<Ability>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassDefinition {
    pub name: String,
    pub description: String,
    pub hit_die: u16,
    #[serde(default)]
    pub casting_ability: Option<Ability>,
    #[serde(default)]
    pub proficiencies: Vec<Proficiency>,
    #[serde(default)]
    pub saving_throws: Vec<Ability>,
    #[serde(default)]
    pub features: Vec<ClassFeature>,
    #[serde(default)]
    pub levels: Vec<ClassLevelRules>,
    #[serde(default)]
    pub subclasses: Vec<SubclassDefinition>,
}

impl ClassDefinition {
    pub fn features(&self, subclass: Option<&str>) -> impl Iterator<Item = &ClassFeature> {
        let sc_features = subclass
            .and_then(|name| self.subclasses.iter().find(|sc| sc.name == name))
            .map(|sc| sc.features.as_slice())
            .unwrap_or_default();
        self.features.iter().chain(sc_features.iter())
    }

    pub fn spells(&self, subclass: Option<&str>) -> impl Iterator<Item = &ClassSpell> {
        self.features(subclass)
            .filter_map(|f| f.spells.as_ref())
            .flat_map(|spells| spells.iter())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub features: Vec<ClassFeature>,
    #[serde(default)]
    pub levels: BTreeMap<u32, SubclassLevelRules>,
}

impl SubclassDefinition {
    pub fn min_level(&self) -> u32 {
        self.levels.keys().next().cloned().unwrap_or(1)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassLevelRules {
    pub level: u32,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassFeature {
    pub name: String,
    pub description: String,
    pub spells: Option<Vec<ClassSpell>>,
    pub fields: Option<Vec<FieldDefinition>>,
}

impl ClassFeature {
    pub fn apply(&self, level: u32, character: &mut Character) {
        let has_feature = character.features.iter().any(|f| f.name == self.name);

        if !has_feature {
            character.features.push(Feature {
                name: self.name.clone(),
                description: self.description.clone(),
            });

            if let Some(spells) = &self.spells {
                let spellcasting = character.spellcasting.get_or_insert_default();
                spellcasting
                    .spells
                    .extend(spells.iter().filter(|s| s.sticky).map(|s| Spell {
                        name: s.name.clone(),
                        description: s.description.clone(),
                        level: s.level,
                        prepared: true,
                        sticky: true,
                    }));
            }
        }

        if let Some(defs) = &self.fields {
            match character.fields.entry(self.name.clone()) {
                Entry::Vacant(place) => {
                    place.insert(
                        defs.iter()
                            .map(|f| FeatureField {
                                name: f.name.clone(),
                                description: f.description.clone(),
                                value: f.kind.to_value(level),
                            })
                            .collect(),
                    );
                }
                Entry::Occupied(mut place) => {
                    let fields = place.get_mut();
                    for (def, field) in defs.iter().zip(fields.iter_mut()) {
                        if field.name != def.name {
                            continue;
                        }

                        match (&def.kind, &mut field.value) {
                            (FieldKind::Die { levels }, FeatureValue::Die(d)) => {
                                *d = get_for_level(levels, level);
                            }
                            (
                                FieldKind::Choice { levels, .. },
                                FeatureValue::Choice { options },
                            ) => {
                                let new_len = get_for_level(levels, level) as usize;
                                if options.len() < new_len {
                                    options.resize(new_len, Default::default());
                                }
                            }
                            (FieldKind::Bonus { levels }, FeatureValue::Bonus(b)) => {
                                *b = get_for_level(levels, level);
                            }
                            (FieldKind::Points { levels }, FeatureValue::Points { max, .. }) => {
                                *max = get_for_level(levels, level);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

fn get_for_level<T: Clone + Default>(levels: &BTreeMap<u32, T>, level: u32) -> T {
    levels
        .range(level..)
        .next()
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(flatten)]
    pub kind: FieldKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind")]
pub enum FieldKind {
    Points {
        #[serde(default)]
        levels: BTreeMap<u32, u32>,
    },
    Choice {
        options: Vec<ChoiceOption>,
        #[serde(default)]
        cost: Option<String>,
        #[serde(default)]
        levels: BTreeMap<u32, u32>,
    },
    Die {
        #[serde(default)]
        levels: BTreeMap<u32, String>,
    },
    Bonus {
        #[serde(default)]
        levels: BTreeMap<u32, i32>,
    },
}

impl FieldKind {
    pub fn to_value(&self, level: u32) -> FeatureValue {
        match self {
            FieldKind::Die { levels } => {
                FeatureValue::Die(levels.get(&level).cloned().unwrap_or_default())
            }
            FieldKind::Choice { levels, .. } => FeatureValue::Choice {
                options: vec![
                    Default::default();
                    levels.get(&level).copied().unwrap_or_default() as usize
                ],
            },
            FieldKind::Bonus { levels } => {
                FeatureValue::Bonus(levels.get(&level).cloned().unwrap_or_default())
            }
            FieldKind::Points { levels } => FeatureValue::Points {
                max: levels.get(&level).copied().unwrap_or_default(),
                used: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChoiceOption {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub level: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassSpell {
    pub name: String,
    pub level: u32,
    pub description: String,
    #[serde(default)]
    pub sticky: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClassLevelRules {
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub spell_slots: Option<Vec<u32>>,
    #[serde(default)]
    pub cantrips_known: Option<u32>,
    #[serde(default)]
    pub spells_known: Option<u32>,
}

impl ClassDefinition {
    pub fn apply(&self, character: &mut Character) {
        let Some(class_level) = character
            .identity
            .classes
            .iter_mut()
            .find(|c| c.class == self.name)
        else {
            return;
        };

        let level = class_level.level;

        if class_level.applied_levels.contains(&level) {
            return;
        }

        class_level.applied_levels.push(class_level.level);
        class_level.hit_die_sides = self.hit_die;

        let Some(rules) = self.levels.get(level as usize - 1) else {
            return;
        };

        let subclass = class_level.subclass.clone();

        for feat in self.features(subclass.as_deref()) {
            if rules.features.contains(&feat.name)
                || character.features.iter().any(|f| f.name == feat.name)
            {
                feat.apply(level, character);
            }
        }

        let Some(spell_slots) = rules.spell_slots.as_ref() else {
            return;
        };

        let spellcasting = character.spellcasting.get_or_insert_default();

        spellcasting
            .spell_slots
            .resize_with(spell_slots.len(), Default::default);
        for (j, &count) in spell_slots.iter().enumerate() {
            spellcasting.spell_slots[j].total = count;
        }
        while spellcasting
            .spell_slots
            .last()
            .is_some_and(|s| s.total == 0 && s.used == 0)
        {
            spellcasting.spell_slots.pop();
        }

        // Ensure enough cantrip lines
        if let Some(n) = rules.cantrips_known {
            let current = spellcasting.cantrips().filter(|s| !s.sticky).count();
            for _ in current..(n as usize) {
                spellcasting.spells.push(Spell {
                    level: 0,
                    ..Default::default()
                });
            }
        }

        // Ensure enough leveled spell lines
        if let Some(n) = rules.spells_known {
            let current = spellcasting.spells().filter(|s| !s.sticky).count();
            let max_spell_level = rules
                .spell_slots
                .as_ref()
                .and_then(|slots| {
                    slots
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, count)| **count > 0)
                        .map(|(i, _)| (i + 1) as u32)
                })
                .unwrap_or(1);
            for _ in current..(n as usize) {
                spellcasting.spells.push(Spell {
                    level: max_spell_level,
                    ..Default::default()
                });
            }
        }

        // Apply hit dice to max HP
        let con_mod = character.ability_modifier(Ability::Constitution);
        let hp_gain = if level == 1 {
            self.hit_die as i32 + con_mod
        } else {
            (self.hit_die as i32) / 2 + 1 + con_mod
        };

        character.combat.hp_max += hp_gain;
        character.combat.hp_current = character.combat.hp_max;
    }
}

// --- Registry ---

#[derive(Clone, Copy)]
pub struct RulesRegistry {
    class_index: LocalResource<Result<Index, String>>,
    class_cache: RwSignal<HashMap<String, ClassDefinition>>,
}

async fn fetch_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let resp = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|error| format!("fetch error: {error}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json()
        .await
        .map_err(|error| format!("parse error: {error}"))
}

impl Default for RulesRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RulesRegistry {
    pub fn new() -> Self {
        let index_url = format!("{BASE_URL}/classes/index.json");
        let class_index = LocalResource::new(move || {
            let url = index_url.clone();
            async move { fetch_json::<Index>(&url).await }
        });

        Self {
            class_index,
            class_cache: RwSignal::new(HashMap::new()),
        }
    }

    pub fn with_class_entries<R>(&self, f: impl FnOnce(&[ClassIndexEntry]) -> R) -> R {
        let guard = self.class_index.read();
        let entries = guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| idx.classes.as_slice());
        f(entries.unwrap_or(&[]))
    }

    pub fn get_class(&self, name: &str) -> Option<ClassDefinition> {
        self.class_cache.read().get(name).cloned()
    }

    pub fn fetch_class(&self, name: &str) {
        if self.class_cache.read().contains_key(name) {
            return;
        }

        let url = {
            let guard = self.class_index.read();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.classes.iter().find(|e| e.name == name) {
                Some(entry) => format!("{BASE_URL}/{}", entry.url),
                None => return,
            }
        };

        let cache = self.class_cache;
        let name = name.to_string();
        leptos::task::spawn_local(async move {
            match fetch_json::<ClassDefinition>(&url).await {
                Ok(def) => {
                    cache.update(|m| {
                        m.insert(name, def);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch class definition: {error}");
                }
            }
        });
    }
}
