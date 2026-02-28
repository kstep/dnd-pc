use std::collections::{BTreeMap, HashMap};

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
                    u32::try_from(v).map(FlexU32).map_err(de::Error::custom)
                }

                fn visit_str<E: de::Error>(self, v: &str) -> Result<FlexU32, E> {
                    v.parse().map(FlexU32).map_err(de::Error::custom)
                }
            }

            d.deserialize_any(V)
        }
    }
}

/// Trait for types that have a `name` field, used by `named_map`.
trait Named {
    fn name(&self) -> &str;
}

/// Deserialize a JSON array `[{"name": "Foo", ...}, ...]` into a
/// `BTreeMap<String, T>` keyed by each element's `name()`.
mod named_map {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer};

    use super::Named;

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<BTreeMap<String, T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de> + Named,
    {
        let vec = Vec::<T>::deserialize(deserializer)?;
        Ok(vec
            .into_iter()
            .map(|item| {
                let key = item.name().to_string();
                (key, item)
            })
            .collect())
    }
}

use crate::{
    BASE_URL,
    model::{
        Ability, Character, CharacterIdentity, ClassLevel, Feature, FeatureField, FeatureValue,
        Proficiency, ProficiencyLevel, RacialTrait, SPELL_SLOT_TABLE, Skill, Spell, SpellData,
    },
    vecset::VecSet,
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

impl Named for RaceTrait {
    fn name(&self) -> &str {
        &self.name
    }
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
    #[serde(default, deserialize_with = "named_map::deserialize")]
    pub traits: BTreeMap<String, RaceTrait>,
    #[serde(default, deserialize_with = "named_map::deserialize")]
    pub features: BTreeMap<String, FeatureDefinition>,
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

        for t in self.traits.values() {
            character.racial_traits.push(RacialTrait {
                name: t.name.clone(),
                description: t.description.clone(),
            });
        }

        let total_level = character.level();
        for feat in self.features.values() {
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
    pub proficiencies: VecSet<Skill>,
    #[serde(default, deserialize_with = "named_map::deserialize")]
    pub features: BTreeMap<String, FeatureDefinition>,
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

        for feat in self.features.values() {
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
    pub proficiencies: VecSet<Proficiency>,
    #[serde(default)]
    pub saving_throws: VecSet<Ability>,
    #[serde(default, deserialize_with = "named_map::deserialize")]
    pub features: BTreeMap<String, FeatureDefinition>,
    #[serde(default)]
    pub levels: Vec<ClassLevelRules>,
    #[serde(default, deserialize_with = "named_map::deserialize")]
    pub subclasses: BTreeMap<String, SubclassDefinition>,
}

impl ClassDefinition {
    pub fn features(&self, subclass: Option<&str>) -> impl Iterator<Item = &FeatureDefinition> {
        let sc_features = subclass
            .and_then(|name| self.subclasses.get(name))
            .into_iter()
            .flat_map(|sc| sc.features.values());
        self.features.values().chain(sc_features)
    }

    pub fn find_feature(&self, name: &str, subclass: Option<&str>) -> Option<&FeatureDefinition> {
        self.features.get(name).or_else(|| {
            subclass
                .and_then(|sc| self.subclasses.get(sc))
                .and_then(|sc| sc.features.get(name))
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassDefinition {
    pub name: String,
    pub description: String,
    #[serde(default, deserialize_with = "named_map::deserialize")]
    pub features: BTreeMap<String, FeatureDefinition>,
    #[serde(default, deserialize_with = "u32_key_map::deserialize")]
    pub levels: BTreeMap<u32, SubclassLevelRules>,
}

impl Named for SubclassDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl SubclassDefinition {
    pub fn min_level(&self) -> u32 {
        self.levels.keys().next().copied().unwrap_or(1)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassLevelRules {
    pub level: u32,
    #[serde(default)]
    pub features: VecSet<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeatureDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub languages: VecSet<String>,
    #[serde(default)]
    pub stackable: bool,
    pub spells: Option<SpellsDefinition>,
    #[serde(default, deserialize_with = "named_map::deserialize")]
    pub fields: BTreeMap<String, FieldDefinition>,
}

impl Named for FeatureDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl FeatureDefinition {
    pub fn apply(&self, level: u32, character: &mut Character) {
        if !character.features.iter().any(|f| f.name == self.name) {
            character.features.push(Feature {
                name: self.name.clone(),
                description: self.description.clone(),
            });
        }

        character.languages.extend(self.languages.iter().cloned());

        if let Some(spells_def) = &self.spells {
            let entry = character.feature_data.entry(self.name.clone()).or_default();
            let sc = entry.spells.get_or_insert_with(|| SpellData {
                casting_ability: spells_def.casting_ability,
                ..Default::default()
            });

            if let Some(rules) = spells_def.levels.get(level as usize - 1) {
                // Cantrips known
                if let Some(n) = rules.cantrips {
                    let current = sc.cantrips().filter(|s| !s.sticky).count();
                    for _ in current..(n as usize) {
                        sc.spells.push(Spell {
                            level: 0,
                            ..Default::default()
                        });
                    }
                }

                // Known spells
                if let Some(n) = rules.spells {
                    let current = sc.spells().filter(|s| !s.sticky).count();
                    let caster_level = character.caster_level() as usize;
                    let max_level = caster_level
                        .checked_sub(1)
                        .and_then(|i| SPELL_SLOT_TABLE.get(i))
                        .map(|row| row.len() as u32)
                        .unwrap_or(1);
                    // Re-borrow sc after the immutable borrow of character.identity above
                    let sc = character
                        .feature_data
                        .get_mut(&self.name)
                        .and_then(|e| e.spells.as_mut())
                        .expect("just inserted");
                    for _ in current..(n as usize) {
                        sc.spells.push(Spell {
                            level: max_level,
                            ..Default::default()
                        });
                    }
                }
            }

            // Sticky spells from inline list
            let sc = character
                .feature_data
                .get_mut(&self.name)
                .and_then(|e| e.spells.as_mut())
                .expect("just inserted");
            if let SpellList::Inline(list) = &spells_def.list {
                for s in list.iter().filter(|s| s.sticky && s.min_level <= level) {
                    if !sc.spells.iter().any(|ex| ex.name == s.name) {
                        sc.spells.push(Spell {
                            name: s.name.clone(),
                            description: s.description.clone(),
                            level: s.level,
                            prepared: true,
                            sticky: true,
                        });
                    }
                }
            }
        }

        if !self.fields.is_empty() {
            let entry = character.feature_data.entry(self.name.clone()).or_default();
            let fields = &mut entry.fields;
            if fields.is_empty() {
                *fields = self
                    .fields
                    .values()
                    .map(|f| FeatureField {
                        name: f.name.clone(),
                        description: f.description.clone(),
                        value: f.kind.to_value(level),
                    })
                    .collect();
            } else {
                for field in fields.iter_mut() {
                    if let Some(def) = self.fields.get(&field.name) {
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
        .range(..=level)
        .next_back()
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

impl Named for FieldDefinition {
    fn name(&self) -> &str {
        &self.name
    }
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
            FieldKind::Die { levels } => FeatureValue::Die(get_for_level(levels, level)),
            FieldKind::Choice { levels, .. } => FeatureValue::Choice {
                options: vec![Default::default(); get_for_level(levels, level) as usize],
            },
            FieldKind::Bonus { levels } => FeatureValue::Bonus(get_for_level(levels, level)),
            FieldKind::Points { levels } => FeatureValue::Points {
                max: get_for_level(levels, level),
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

impl Named for SpellDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpellsDefinition {
    pub casting_ability: Ability,
    #[serde(default)]
    pub caster_coef: u8,
    #[serde(default)]
    pub list: SpellList,
    #[serde(default)]
    pub levels: Vec<SpellLevelRules>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SpellList {
    Ref { from: String },
    Inline(Vec<SpellDefinition>),
}

impl Default for SpellList {
    fn default() -> Self {
        Self::Inline(Vec::new())
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClassLevelRules {
    #[serde(default)]
    pub features: VecSet<String>,
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

        if !class_level.applied_levels.insert(level) {
            return;
        }
        class_level.hit_die_sides = self.hit_die;

        // Apply saving throws and proficiencies at level 1
        if level == 1 {
            character
                .saving_throws
                .extend(self.saving_throws.iter().copied());
            character
                .proficiencies
                .extend(self.proficiencies.iter().copied());
        }

        let Some(rules) = self.levels.get(level as usize - 1) else {
            return;
        };

        let subclass = class_level.subclass.clone();

        // Set caster_coef from the class's spellcasting feature (if any)
        let caster_coef = self
            .features(subclass.as_deref())
            .filter_map(|f| f.spells.as_ref())
            .map(|s| s.caster_coef)
            .max()
            .unwrap_or(0);
        if let Some(cl) = character
            .identity
            .classes
            .iter_mut()
            .find(|c| c.class == self.name)
        {
            cl.caster_coef = caster_coef;
        }

        for feat in self.features(subclass.as_deref()) {
            let is_new = rules.features.contains(&feat.name);
            let already_has = character.features.iter().any(|f| f.name == feat.name);

            // Skip non-stackable features that would be granted again from another source
            if is_new && already_has && !feat.stackable {
                continue;
            }

            if is_new || already_has {
                feat.apply(level, character);
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
    spell_list_cache: RwSignal<HashMap<String, Vec<SpellDefinition>>>,
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
            spell_list_cache: RwSignal::new(HashMap::new()),
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

    pub fn has_class(&self, name: &str) -> bool {
        self.class_cache.read().contains_key(name)
    }

    pub fn with_class<R>(&self, name: &str, f: impl FnOnce(&ClassDefinition) -> R) -> Option<R> {
        self.class_cache.read().get(name).map(f)
    }

    pub fn fetch_spell_list(&self, path: &str) {
        if self.spell_list_cache.read().contains_key(path) {
            return;
        }

        let url = format!("{BASE_URL}/{path}");
        let cache = self.spell_list_cache;
        let path = path.to_string();
        leptos::task::spawn_local(async move {
            match fetch_json::<Vec<SpellDefinition>>(&url).await {
                Ok(list) => {
                    cache.update(|m| {
                        m.insert(path, list);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch spell list: {error}");
                }
            }
        });
    }

    pub fn with_spell_list<R>(
        &self,
        list: &SpellList,
        f: impl FnOnce(&[SpellDefinition]) -> R,
    ) -> R {
        match list {
            SpellList::Inline(spells) => f(spells),
            SpellList::Ref { from } => {
                self.fetch_spell_list(from);
                let cache = self.spell_list_cache.read();
                f(cache.get(from.as_str()).map_or(&[], |v| v.as_slice()))
            }
        }
    }

    /// Find a feature definition by name, searching class, background, and race
    /// definitions. Applies callback on a reference — no cloning.
    pub fn with_feature<R>(
        &self,
        identity: &CharacterIdentity,
        feature_name: &str,
        f: impl FnOnce(&FeatureDefinition) -> R,
    ) -> Option<R> {
        let class_cache = self.class_cache.read();
        let bg_cache = self.background_cache.read();
        let race_cache = self.race_cache.read();

        for cl in &identity.classes {
            if let Some(def) = class_cache.get(&cl.class)
                && let Some(feat) = def.find_feature(feature_name, cl.subclass.as_deref())
            {
                return Some(f(feat));
            }
        }

        if let Some(feat) = bg_cache
            .get(&identity.background)
            .and_then(|def| def.features.get(feature_name))
        {
            return Some(f(feat));
        }

        if let Some(feat) = race_cache
            .get(&identity.race)
            .and_then(|def| def.features.get(feature_name))
        {
            return Some(f(feat));
        }

        None
    }

    /// Return the class level for the class that owns the given feature.
    /// Returns `None` if the feature is not a class feature (e.g.
    /// background/race).
    pub fn feature_class_level(
        &self,
        identity: &CharacterIdentity,
        feature_name: &str,
    ) -> Option<u32> {
        let class_cache = self.class_cache.read();
        identity.classes.iter().find_map(|cl| {
            let def = class_cache.get(&cl.class)?;
            def.find_feature(feature_name, cl.subclass.as_deref())
                .map(|_| cl.level)
        })
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
            if let Some(def) = cache.get(&cl.class)
                && let Some(feat) = def.find_feature(feature_name, cl.subclass.as_deref())
                && let Some(field_def) = feat.fields.get(field_name)
            {
                return Self::resolve_choice_options(field_def, character_fields);
            }
        }
        Vec::new()
    }

    fn resolve_choice_options(
        field_def: &FieldDefinition,
        character_fields: &[FeatureField],
    ) -> Vec<ChoiceOption> {
        if let FieldKind::Choice { options, .. } = &field_def.kind {
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
            if let Some(def) = cache.get(&cl.class)
                && let Some(feat) = def.find_feature(feature_name, cl.subclass.as_deref())
                && let Some(fd) = feat.fields.get(field_name)
                && let FieldKind::Choice { cost, .. } = &fd.kind
            {
                return cost.clone();
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

    pub fn has_race(&self, name: &str) -> bool {
        self.race_cache.read().contains_key(name)
    }

    pub fn with_race<R>(&self, name: &str, f: impl FnOnce(&RaceDefinition) -> R) -> Option<R> {
        self.race_cache.read().get(name).map(f)
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

    pub fn has_background(&self, name: &str) -> bool {
        self.background_cache.read().contains_key(name)
    }

    pub fn with_background<R>(
        &self,
        name: &str,
        f: impl FnOnce(&BackgroundDefinition) -> R,
    ) -> Option<R> {
        self.background_cache.read().get(name).map(f)
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

    /// Fill empty descriptions on a character from registry definitions.
    /// Reads registry caches (providing reactive tracking) and only writes
    /// when a description is empty and the registry has a non-empty one.
    pub fn fill_descriptions(&self, character: &mut Character) {
        let class_cache = self.class_cache.read();
        let bg_cache = self.background_cache.read();
        let race_cache = self.race_cache.read();

        // Helper: find a FeatureDefinition by name across all sources.
        // Duplicates with_feature() logic to reuse pre-acquired read guards.
        let find_feat = |name: &str| -> Option<&FeatureDefinition> {
            for cl in &character.identity.classes {
                if let Some(def) = class_cache.get(&cl.class)
                    && let Some(feat) = def.find_feature(name, cl.subclass.as_deref())
                {
                    return Some(feat);
                }
            }
            if let Some(feat) = bg_cache
                .get(&character.identity.background)
                .and_then(|def| def.features.get(name))
            {
                return Some(feat);
            }
            race_cache
                .get(&character.identity.race)
                .and_then(|def| def.features.get(name))
        };

        // Feature descriptions
        for feature in &mut character.features {
            if feature.description.is_empty()
                && !feature.name.is_empty()
                && let Some(feat_def) = find_feat(&feature.name)
                && !feat_def.description.is_empty()
            {
                feature.description = feat_def.description.clone();
            }
        }

        // Racial trait descriptions
        if let Some(race_def) = race_cache.get(&character.identity.race) {
            for racial_trait in &mut character.racial_traits {
                if racial_trait.description.is_empty()
                    && !racial_trait.name.is_empty()
                    && let Some(def_trait) = race_def.traits.get(&racial_trait.name)
                    && !def_trait.description.is_empty()
                {
                    racial_trait.description = def_trait.description.clone();
                }
            }
        }

        // Trigger spell list fetches before acquiring the read guard
        for key in character.feature_data.keys() {
            if let Some(feat_def) = find_feat(key)
                && let Some(spells_def) = &feat_def.spells
                && let SpellList::Ref { from } = &spells_def.list
            {
                self.fetch_spell_list(from);
            }
        }

        // Feature data entries: fields, choices, spells
        let spell_list_cache = self.spell_list_cache.read();

        for (key, entry) in &mut character.feature_data {
            let Some(feat_def) = find_feat(key) else {
                continue;
            };

            // Field descriptions and choice option descriptions
            if !feat_def.fields.is_empty() {
                for field in &mut entry.fields {
                    if let Some(field_def) = feat_def.fields.get(&field.name) {
                        if field.description.is_empty() && !field_def.description.is_empty() {
                            field.description = field_def.description.clone();
                        }

                        if let FieldKind::Choice { options, .. } = &field_def.kind {
                            let def_options = match options {
                                ChoiceOptions::List(list) => list.as_slice(),
                                ChoiceOptions::Ref { .. } => &[],
                            };
                            for opt in field.value.choices_mut() {
                                if opt.description.is_empty()
                                    && !opt.name.is_empty()
                                    && let Some(def_opt) =
                                        def_options.iter().find(|o| o.name == opt.name)
                                    && !def_opt.description.is_empty()
                                {
                                    opt.description = def_opt.description.clone();
                                }
                            }
                        }
                    }
                }
            }

            // Spell descriptions
            if let Some(spells_def) = &feat_def.spells
                && let Some(spell_data) = &mut entry.spells
            {
                for spell in &mut spell_data.spells {
                    if spell.description.is_empty() && !spell.name.is_empty() {
                        let spell_defs: &[SpellDefinition] = match &spells_def.list {
                            SpellList::Inline(spells) => spells,
                            SpellList::Ref { from } => spell_list_cache
                                .get(from.as_str())
                                .map_or(&[], |v| v.as_slice()),
                        };
                        let desc = spell_defs
                            .iter()
                            .find(|s| s.name == spell.name)
                            .map(|s| &s.description);
                        if let Some(desc) = desc
                            && !desc.is_empty()
                        {
                            spell.description = desc.clone();
                        }
                    }
                }
            }
        }
    }
}
