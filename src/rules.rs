use std::collections::{BTreeMap, HashMap, btree_map::Entry};

use leptos::prelude::*;
use serde::Deserialize;

/// Deserialize a `BTreeMap<u32, V>` accepting both numeric keys (binary
/// formats) and stringified numbers (JSON).
mod u32_key_map {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, de};

    pub fn deserialize<'de, D, V>(deserializer: D) -> Result<BTreeMap<u32, V>, D::Error>
    where
        D: Deserializer<'de>,
        V: Deserialize<'de>,
    {
        struct Visitor<V>(std::marker::PhantomData<V>);

        impl<'de, V: Deserialize<'de>> de::Visitor<'de> for Visitor<V> {
            type Value = BTreeMap<u32, V>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a map with u32 keys (numeric or stringified)")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut result = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<FlexU32, V>()? {
                    result.insert(key.0, value);
                }
                Ok(result)
            }
        }

        deserializer.deserialize_map(Visitor(std::marker::PhantomData))
    }

    struct FlexU32(u32);

    impl<'de> Deserialize<'de> for FlexU32 {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;

            impl<'de> de::Visitor<'de> for V {
                type Value = FlexU32;

                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("u32 or stringified u32")
                }

                fn visit_u32<E: de::Error>(self, v: u32) -> Result<FlexU32, E> {
                    Ok(FlexU32(v))
                }

                fn visit_u64<E: de::Error>(self, v: u64) -> Result<FlexU32, E> {
                    Ok(FlexU32(v as u32))
                }

                fn visit_str<E: de::Error>(self, v: &str) -> Result<FlexU32, E> {
                    v.parse().map(FlexU32).map_err(de::Error::custom)
                }
            }

            d.deserialize_any(V)
        }
    }
}

use crate::{
    BASE_URL,
    model::{
        Ability, Character, ClassLevel, Feature, FeatureField, FeatureValue, Proficiency,
        ProficiencyLevel, RacialTrait, Skill, Spell, SpellcastingData,
    },
};

// --- JSON types ---

#[derive(Debug, Clone, Deserialize)]
struct Index {
    classes: Vec<ClassIndexEntry>,
    #[serde(default)]
    races: Vec<RaceIndexEntry>,
    #[serde(default)]
    backgrounds: Vec<BackgroundIndexEntry>,
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
pub struct RaceIndexEntry {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RaceTrait {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AbilityModifier {
    pub ability: Ability,
    pub modifier: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RaceDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub speed: u32,
    #[serde(default)]
    pub ability_modifiers: Vec<AbilityModifier>,
    #[serde(default)]
    pub traits: Vec<RaceTrait>,
    #[serde(default)]
    pub features: Vec<FeatureDefinition>,
}

impl RaceDefinition {
    pub fn apply(&self, character: &mut Character) {
        if self.speed > 0 {
            character.combat.speed = self.speed;
        }

        for am in &self.ability_modifiers {
            let current = character.abilities.get(am.ability) as i32;
            character
                .abilities
                .set(am.ability, (current + am.modifier).max(1) as u32);
        }

        for t in &self.traits {
            character.racial_traits.push(RacialTrait {
                name: t.name.clone(),
                description: t.description.clone(),
            });
        }

        let total_level = character.level();
        for feat in &self.features {
            feat.apply(total_level, character);
        }

        character.identity.race_applied = true;
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundIndexEntry {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub ability_modifiers: Vec<AbilityModifier>,
    #[serde(default)]
    pub proficiencies: Vec<Skill>,
    #[serde(default)]
    pub features: Vec<FeatureDefinition>,
}

impl BackgroundDefinition {
    pub fn apply(&self, character: &mut Character) {
        for am in &self.ability_modifiers {
            let current = character.abilities.get(am.ability) as i32;
            character
                .abilities
                .set(am.ability, (current + am.modifier).max(1) as u32);
        }

        for &skill in &self.proficiencies {
            character
                .skills
                .entry(skill)
                .or_insert(ProficiencyLevel::Proficient);
        }

        for feat in &self.features {
            feat.apply(1, character);
        }

        character.identity.background_applied = true;
    }
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
    pub features: Vec<FeatureDefinition>,
    #[serde(default)]
    pub levels: Vec<ClassLevelRules>,
    #[serde(default)]
    pub subclasses: Vec<SubclassDefinition>,
}

impl ClassDefinition {
    pub fn features(&self, subclass: Option<&str>) -> impl Iterator<Item = &FeatureDefinition> {
        let sc_features = subclass
            .and_then(|name| self.subclasses.iter().find(|sc| sc.name == name))
            .map(|sc| sc.features.as_slice())
            .unwrap_or_default();
        self.features.iter().chain(sc_features.iter())
    }

    pub fn spells(&self, subclass: Option<&str>) -> impl Iterator<Item = &SpellDefinition> {
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
    pub features: Vec<FeatureDefinition>,
    #[serde(default, deserialize_with = "u32_key_map::deserialize")]
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
pub struct FeatureDefinition {
    pub name: String,
    pub description: String,
    pub spells: Option<Vec<SpellDefinition>>,
    pub fields: Option<Vec<FieldDefinition>>,
}

impl FeatureDefinition {
    pub fn apply(&self, level: u32, character: &mut Character) {
        if !character.features.iter().any(|f| f.name == self.name) {
            character.features.push(Feature {
                name: self.name.clone(),
                description: self.description.clone(),
            });
        }

        if let Some(spells) = &self.spells {
            let spellcasting = character.spellcasting.get_or_insert_default();
            for s in spells.iter().filter(|s| s.sticky && s.min_level <= level) {
                if !spellcasting.spells.iter().any(|ex| ex.name == s.name) {
                    spellcasting.spells.push(Spell {
                        name: s.name.clone(),
                        description: s.description.clone(),
                        level: s.level,
                        prepared: true,
                        sticky: true,
                    });
                }
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
        #[serde(default, deserialize_with = "u32_key_map::deserialize")]
        levels: BTreeMap<u32, u32>,
    },
    Choice {
        #[serde(default)]
        options: ChoiceOptions,
        #[serde(default)]
        cost: Option<String>,
        #[serde(default, deserialize_with = "u32_key_map::deserialize")]
        levels: BTreeMap<u32, u32>,
    },
    Die {
        #[serde(default, deserialize_with = "u32_key_map::deserialize")]
        levels: BTreeMap<u32, String>,
    },
    Bonus {
        #[serde(default, deserialize_with = "u32_key_map::deserialize")]
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
#[serde(untagged)]
pub enum ChoiceOptions {
    List(Vec<ChoiceOption>),
    Ref { from: String },
}

impl Default for ChoiceOptions {
    fn default() -> Self {
        Self::List(Vec::new())
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
pub struct SpellDefinition {
    pub name: String,
    pub level: u32,
    pub description: String,
    #[serde(default)]
    pub sticky: bool,
    #[serde(default)]
    pub min_level: u32,
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
    pub fn apply_level(&self, level: u32, character: &mut Character) {
        let Some(class_level) = character
            .identity
            .classes
            .iter_mut()
            .find(|c| c.class == self.name)
        else {
            return;
        };

        if class_level.applied_levels.contains(&level) {
            return;
        }

        class_level.applied_levels.push(level);
        class_level.hit_die_sides = self.hit_die;

        // Apply saving throws and proficiencies at level 1
        if level == 1 {
            for &ability in &self.saving_throws {
                character.saving_throws.insert(ability);
            }
            for &prof in &self.proficiencies {
                character.proficiencies.insert(prof);
            }
            if let Some(ability) = self.casting_ability {
                character
                    .spellcasting
                    .get_or_insert_with(|| SpellcastingData {
                        casting_ability: ability,
                        ..Default::default()
                    });
            }
        }

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

        // Update spell slots
        if let Some(ref spell_slots) = rules.spell_slots {
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
        }

        // Ensure enough cantrip lines
        if let Some(n) = rules.cantrips_known {
            let spellcasting = character.spellcasting.get_or_insert_default();
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
            let spellcasting = character.spellcasting.get_or_insert_default();
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
    race_cache: RwSignal<HashMap<String, RaceDefinition>>,
    background_cache: RwSignal<HashMap<String, BackgroundDefinition>>,
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
        let index_url = format!("{BASE_URL}/index.json");
        let class_index = LocalResource::new(move || {
            let url = index_url.clone();
            async move { fetch_json::<Index>(&url).await }
        });

        Self {
            class_index,
            class_cache: RwSignal::new(HashMap::new()),
            race_cache: RwSignal::new(HashMap::new()),
            background_cache: RwSignal::new(HashMap::new()),
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

    pub fn get_choice_options(
        &self,
        classes: &[ClassLevel],
        feature_name: &str,
        field_name: &str,
        character_fields: &[FeatureField],
    ) -> Vec<ChoiceOption> {
        let cache = self.class_cache.read();
        for cl in classes {
            if let Some(def) = cache.get(&cl.class) {
                for feat in def.features(cl.subclass.as_deref()) {
                    if feat.name != feature_name {
                        continue;
                    }
                    if let Some(fields) = &feat.fields {
                        return Self::resolve_choice_options(fields, field_name, character_fields);
                    }
                }
            }
        }
        Vec::new()
    }

    fn resolve_choice_options(
        fields: &[FieldDefinition],
        field_name: &str,
        character_fields: &[FeatureField],
    ) -> Vec<ChoiceOption> {
        for fd in fields {
            if fd.name != field_name {
                continue;
            }
            if let FieldKind::Choice { options, .. } = &fd.kind {
                return match options {
                    ChoiceOptions::List(list) => list.clone(),
                    ChoiceOptions::Ref { from } => character_fields
                        .iter()
                        .find(|cf| cf.name == *from)
                        .into_iter()
                        .flat_map(|cf| cf.value.choices())
                        .filter(|o| !o.name.is_empty())
                        .map(|o| ChoiceOption {
                            name: o.name.clone(),
                            description: o.description.clone(),
                            cost: o.cost,
                            level: 0,
                        })
                        .collect(),
                };
            }
        }
        Vec::new()
    }

    pub fn get_choice_cost_label(
        &self,
        classes: &[ClassLevel],
        feature_name: &str,
        field_name: &str,
    ) -> Option<String> {
        let cache = self.class_cache.read();
        for cl in classes {
            if let Some(def) = cache.get(&cl.class) {
                for feat in def.features(cl.subclass.as_deref()) {
                    if feat.name != feature_name {
                        continue;
                    }
                    if let Some(fields) = &feat.fields {
                        for fd in fields {
                            if fd.name == field_name
                                && let FieldKind::Choice { cost, .. } = &fd.kind
                            {
                                return cost.clone();
                            }
                        }
                    }
                }
            }
        }
        None
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

    pub fn with_race_entries<R>(&self, f: impl FnOnce(&[RaceIndexEntry]) -> R) -> R {
        let guard = self.class_index.read();
        let entries = guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| idx.races.as_slice());
        f(entries.unwrap_or(&[]))
    }

    pub fn get_race(&self, name: &str) -> Option<RaceDefinition> {
        self.race_cache.read().get(name).cloned()
    }

    pub fn fetch_race(&self, name: &str) {
        if self.race_cache.read().contains_key(name) {
            return;
        }

        let url = {
            let guard = self.class_index.read();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.races.iter().find(|e| e.name == name) {
                Some(entry) => format!("{BASE_URL}/{}", entry.url),
                None => return,
            }
        };

        let cache = self.race_cache;
        let name = name.to_string();
        leptos::task::spawn_local(async move {
            match fetch_json::<RaceDefinition>(&url).await {
                Ok(def) => {
                    cache.update(|m| {
                        m.insert(name, def);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch race definition: {error}");
                }
            }
        });
    }

    pub fn with_background_entries<R>(&self, f: impl FnOnce(&[BackgroundIndexEntry]) -> R) -> R {
        let guard = self.class_index.read();
        let entries = guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| idx.backgrounds.as_slice());
        f(entries.unwrap_or(&[]))
    }

    pub fn get_background(&self, name: &str) -> Option<BackgroundDefinition> {
        self.background_cache.read().get(name).cloned()
    }

    pub fn fetch_background(&self, name: &str) {
        if self.background_cache.read().contains_key(name) {
            return;
        }

        let url = {
            let guard = self.class_index.read();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.backgrounds.iter().find(|e| e.name == name) {
                Some(entry) => format!("{BASE_URL}/{}", entry.url),
                None => return,
            }
        };

        let cache = self.background_cache;
        let name = name.to_string();
        leptos::task::spawn_local(async move {
            match fetch_json::<BackgroundDefinition>(&url).await {
                Ok(def) => {
                    cache.update(|m| {
                        m.insert(name, def);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch background definition: {error}");
                }
            }
        });
    }
}
