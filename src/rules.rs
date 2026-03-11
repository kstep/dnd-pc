use std::collections::{BTreeMap, HashMap, HashSet};

use leptos::prelude::*;
use serde::Deserialize;

use crate::{
    BASE_URL,
    demap::{self, Named},
    expr::Expr,
    model::{
        Ability, Attribute, Character, CharacterIdentity, ClassLevel, Die, Feature, FeatureField,
        FeatureSource, FeatureValue, FreeUses, Proficiency, ProficiencyLevel, RacialTrait, Skill,
        Spell, SpellData, SpellSlotPool,
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
    #[serde(default)]
    spells: Vec<SpellIndexEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpellIndexEntry {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub url: String,
}

impl SpellIndexEntry {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassIndexEntry {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub prerequisites: Vec<Ability>,
}

impl ClassIndexEntry {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RaceIndexEntry {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub url: String,
    #[serde(default)]
    pub description: String,
}

impl RaceIndexEntry {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RaceTrait {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub description: String,
    #[serde(default)]
    pub languages: VecSet<String>,
}

impl RaceTrait {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
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
    #[serde(default)]
    pub label: Option<String>,
    pub description: String,
    #[serde(default)]
    pub speed: u32,
    #[serde(default)]
    pub ability_modifiers: Vec<AbilityModifier>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub traits: BTreeMap<String, RaceTrait>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub features: BTreeMap<String, FeatureDefinition>,
}

impl RaceDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

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
                label: t.label.clone(),
                description: t.description.clone(),
            });
            character.languages.extend(t.languages.iter().cloned());
        }

        let total_level = character.level();
        let source = FeatureSource::Race(self.name.clone());
        for feat in self.features.values() {
            feat.apply(total_level, character, &source);
        }

        character.identity.race_applied = true;
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundIndexEntry {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub url: String,
    #[serde(default)]
    pub description: String,
}

impl BackgroundIndexEntry {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub description: String,
    #[serde(default)]
    pub ability_modifiers: Vec<AbilityModifier>,
    #[serde(default)]
    pub proficiencies: VecSet<Skill>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub features: BTreeMap<String, FeatureDefinition>,
}

impl BackgroundDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

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

        let source = FeatureSource::Background(self.name.clone());
        for feat in self.features.values() {
            feat.apply(1, character, &source);
        }

        character.identity.background_applied = true;
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub description: String,
    pub hit_die: u32,
    #[serde(default)]
    pub proficiencies: VecSet<Proficiency>,
    #[serde(default)]
    pub saving_throws: VecSet<Ability>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub features: BTreeMap<String, FeatureDefinition>,
    #[serde(default)]
    pub levels: Vec<ClassLevelRules>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub subclasses: BTreeMap<String, SubclassDefinition>,
}

impl ClassDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

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
    #[serde(default)]
    pub label: Option<String>,
    pub description: String,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub features: BTreeMap<String, FeatureDefinition>,
    #[serde(default, deserialize_with = "demap::u32_key_map")]
    pub levels: BTreeMap<u32, SubclassLevelRules>,
}

impl Named for SubclassDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl SubclassDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

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
    #[serde(default)]
    pub label: Option<String>,
    pub description: String,
    #[serde(default)]
    pub languages: VecSet<String>,
    #[serde(default)]
    pub stackable: bool,
    pub spells: Option<SpellsDefinition>,
    #[serde(default, deserialize_with = "demap::named_map")]
    pub fields: BTreeMap<String, FieldDefinition>,
    #[serde(default)]
    pub assign: Option<Vec<Assignment>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Assignment {
    pub expr: Expr<Attribute>,
    pub when: WhenCondition,
}

#[derive(Debug, Copy, Clone, Deserialize, PartialEq, Eq)]
pub enum WhenCondition {
    Always,
    OnFeatureAdd,
    OnLevelUp,
    OnLongRest,
    OnShortRest,
}

impl Named for FeatureDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl FeatureDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    /// Returns `(cost_field_name, short_suffix)` if this feature has a
    /// spells cost backed by a Points field (e.g. Sorcery Points → "SP").
    pub fn cost_info(&self) -> Option<(&str, &str)> {
        let cost_name = self.spells.as_ref()?.cost.as_deref()?;
        let field_def = self.fields.get(cost_name)?;
        let short = match &field_def.kind {
            FieldKind::Points { short, .. } => short.as_deref()?,
            _ => return None,
        };
        Some((cost_name, short))
    }

    /// Resolve `ChoiceOptions` to definition options, following `Ref` links
    /// within this feature's fields.
    fn resolve_def_options<'a>(&'a self, options: &'a ChoiceOptions) -> &'a [ChoiceOption] {
        match options {
            ChoiceOptions::List(list) => list.as_slice(),
            ChoiceOptions::Ref { from } => self
                .fields
                .get(from.as_str())
                .and_then(|ref_fd| match &ref_fd.kind {
                    FieldKind::Choice {
                        options: ChoiceOptions::List(list),
                        ..
                    } => Some(list.as_slice()),
                    _ => None,
                })
                .unwrap_or(&[]),
        }
    }

    pub fn assign(&self, character: &mut Character, when: WhenCondition) {
        log::info!(
            "Checking assignments for feature '{}', when condition: {:?}",
            self.name,
            when,
        );

        let Some(assign) = &self.assign else { return };

        assign.iter().filter(|a| a.when == when).for_each(|a| {
            log::info!(
                "Applying assignment for feature '{}': {:?} (when: {:?})",
                self.name,
                a.expr,
                a.when,
            );
            match a.expr.apply(character) {
                Ok(value) => {
                    log::info!(
                        "Result of assignment expression for feature '{}': {value:?}",
                        self.name,
                    );
                }
                Err(error) => {
                    log::error!(
                        "Failed to apply assignment for feature '{}': {error:?}",
                        self.name,
                    );
                }
            }
        });
    }

    pub fn apply(&self, level: u32, character: &mut Character, source: &FeatureSource) {
        if character.features.iter().any(|f| f.name == self.name) {
            self.assign(character, WhenCondition::OnLevelUp);
        } else {
            character.features.push(Feature {
                name: self.name.clone(),
                label: self.label.clone(),
                description: self.description.clone(),
            });

            self.assign(character, WhenCondition::OnFeatureAdd);
        }

        character.languages.extend(self.languages.iter().cloned());

        if let Some(spells_def) = &self.spells {
            // Compute before taking mutable borrow on feature_data
            let highest_slot_level = character.highest_spell_slot_level(spells_def.pool);

            let entry = character.feature_data.entry(self.name.clone()).or_default();
            entry.source = Some(source.clone());
            let spell_data = entry.spells.get_or_insert_with(|| SpellData {
                casting_ability: spells_def.casting_ability,
                caster_coef: spells_def.caster_coef,
                pool: spells_def.pool,
                ..Default::default()
            });

            if let Some(rules) = spells_def.levels.get(level as usize - 1) {
                let (cantrips_current, spells_current) = spell_data
                    .spells
                    .iter()
                    .filter(|s| !s.sticky)
                    .fold((0usize, 0usize), |(c, s), spell| {
                        if spell.level == 0 {
                            (c + 1, s)
                        } else {
                            (c, s + 1)
                        }
                    });

                spell_data.spells.extend(
                    (cantrips_current..rules.cantrips.unwrap_or(cantrips_current as u32) as usize)
                        .map(|_| Spell {
                            level: 0,
                            ..Default::default()
                        })
                        .chain(
                            (spells_current
                                ..rules.spells.unwrap_or(spells_current as u32) as usize)
                                .map(|_| Spell {
                                    level: highest_slot_level,
                                    ..Default::default()
                                }),
                        ),
                );
            }

            // Resolve FreeUses max from FreeUses field definition
            let free_uses_max = self
                .fields
                .values()
                .find_map(|f| match &f.kind {
                    FieldKind::FreeUses { levels } => Some(get_for_level(levels, level)),
                    _ => None,
                })
                .unwrap_or_default();

            // Sticky spells from inline list
            if let SpellList::Inline(list) = &spells_def.list {
                for s in list.iter().filter(|s| s.sticky && s.min_level <= level) {
                    if !spell_data.spells.iter().any(|ex| ex.name == s.name) {
                        let free_uses = if s.cost > 0 && free_uses_max > 0 {
                            Some(FreeUses {
                                used: 0,
                                max: free_uses_max,
                            })
                        } else {
                            None
                        };
                        spell_data.spells.push(Spell {
                            name: s.name.clone(),
                            label: s.label.clone(),
                            description: s.description.clone(),
                            level: s.level,
                            prepared: true,
                            sticky: true,
                            cost: s.cost,
                            free_uses,
                        });
                    }
                }
            }

            // Update free_uses.max on existing spells (level-up)
            if free_uses_max > 0 {
                for spell in &mut spell_data.spells {
                    if spell.cost > 0 {
                        if let Some(fu) = &mut spell.free_uses {
                            fu.max = free_uses_max;
                        } else {
                            spell.free_uses = Some(FreeUses {
                                used: 0,
                                max: free_uses_max,
                            });
                        }
                    }
                }
            }
        } else {
            // Even without spells, set source on feature_data if it exists
            if let Some(entry) = character.feature_data.get_mut(&self.name) {
                entry.source = Some(source.clone());
            }
        }

        if !self.fields.is_empty() {
            let entry = character.feature_data.entry(self.name.clone()).or_default();
            if entry.source.is_none() {
                entry.source = Some(source.clone());
            }
            let fields = &mut entry.fields;
            if fields.is_empty() {
                *fields = self
                    .fields
                    .values()
                    .filter(|f| !matches!(f.kind, FieldKind::FreeUses { .. }))
                    .map(|f| FeatureField {
                        name: f.name.clone(),
                        label: f.label.clone(),
                        description: f.description.clone(),
                        value: f.kind.to_value(level),
                    })
                    .collect();
            } else {
                for field in fields.iter_mut() {
                    if let Some(def) = self.fields.get(&field.name) {
                        match (&def.kind, &mut field.value) {
                            (FieldKind::Die { levels }, FeatureValue::Die { die, .. }) => {
                                *die = get_for_level(levels, level);
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
                            (
                                FieldKind::Points { levels, .. },
                                FeatureValue::Points { max, .. },
                            ) => {
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

pub fn get_for_level<T: Clone + Default>(levels: &BTreeMap<u32, T>, level: u32) -> T {
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
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(flatten)]
    pub kind: FieldKind,
}

impl FieldDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
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
        #[serde(default)]
        short: Option<String>,
        #[serde(default, deserialize_with = "demap::u32_key_map")]
        levels: BTreeMap<u32, u32>,
    },
    Choice {
        #[serde(default)]
        options: ChoiceOptions,
        #[serde(default)]
        cost: Option<String>,
        #[serde(default, deserialize_with = "demap::u32_key_map")]
        levels: BTreeMap<u32, u32>,
    },
    Die {
        #[serde(default, deserialize_with = "demap::u32_key_map")]
        levels: BTreeMap<u32, Die>,
    },
    Bonus {
        #[serde(default, deserialize_with = "demap::u32_key_map")]
        levels: BTreeMap<u32, i32>,
    },
    FreeUses {
        #[serde(default, deserialize_with = "demap::u32_key_map")]
        levels: BTreeMap<u32, u32>,
    },
}

impl FieldKind {
    pub fn to_value(&self, level: u32) -> FeatureValue {
        match self {
            FieldKind::Die { levels } => FeatureValue::Die {
                die: get_for_level(levels, level),
                used: 0,
            },
            FieldKind::Choice { levels, .. } => FeatureValue::Choice {
                options: vec![Default::default(); get_for_level(levels, level) as usize],
            },
            FieldKind::Bonus { levels } => FeatureValue::Bonus(get_for_level(levels, level)),
            FieldKind::Points { levels, .. } => FeatureValue::Points {
                max: get_for_level(levels, level),
                used: 0,
            },
            FieldKind::FreeUses { .. } => {
                unreachable!("FreeUses fields are not converted to FeatureValue")
            }
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
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub level: u32,
}

impl ChoiceOption {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpellDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    pub level: u32,
    pub description: String,
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
    /// Apply all unapplied levels (1..=class_level) in order.
    ///
    /// This is the single entry point for level-up. For each unapplied level it:
    /// 1. Updates spell slots (so features can read highest slot level)
    /// 2. Applies class features, saving throws, proficiencies, HP
    /// 3. Re-applies race features (unlocks level-gated racial spells)
    /// 4. Updates spell slots again (needed when SpellData was just created)
    pub fn apply_level(&self, character: &mut Character, registry: &RulesRegistry) {
        let Some(class_level) = character
            .identity
            .classes
            .iter()
            .find(|c| c.class == self.name)
        else {
            return;
        };

        let current_level = class_level.level;
        let unapplied: Vec<u32> = (1..=current_level)
            .filter(|lvl| !class_level.applied_levels.contains(lvl))
            .collect();

        if unapplied.is_empty() {
            return;
        }

        let subclass = class_level.subclass.clone();

        for level in unapplied {
            // Collect per-pool slot overrides for this level
            let pool_slots: Vec<_> = {
                let mut seen_pools = HashSet::new();
                self.features(subclass.as_deref())
                    .filter_map(|feature| {
                        let spells_def = feature.spells.as_ref()?;
                        seen_pools.insert(spells_def.pool).then(|| {
                            let slots = spells_def
                                .levels
                                .get(level as usize - 1)
                                .and_then(|lr| lr.slots.clone());
                            (spells_def.pool, slots)
                        })
                    })
                    .collect()
            };

            // Update spell slots before applying features (so feat.apply()
            // can read highest_spell_slot_level)
            for (pool, slots) in &pool_slots {
                character.update_spell_slots(*pool, slots.as_deref());
            }

            self.apply_single_level(level, character);

            // Re-apply race features at new total level (unlocks level-gated
            // racial spells)
            if character.identity.race_applied {
                let race_name = character.identity.race.clone();
                let source = FeatureSource::Race(race_name.clone());
                let total_level = character.level();
                registry.with_race(&race_name, |race_def| {
                    for feat in race_def.features.values() {
                        feat.apply(total_level, character, &source);
                    }
                });
            }

            // Update spell slots again (needed for first level when SpellData
            // was just created by apply_single_level)
            for (pool, slots) in &pool_slots {
                character.update_spell_slots(*pool, slots.as_deref());
            }
        }
    }

    /// Apply a single class level's effects. Called by `apply_level` for each
    /// unapplied level in sequence.
    fn apply_single_level(&self, level: u32, character: &mut Character) {
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
        let source = FeatureSource::Class(self.name.clone());

        let subclass_rules = subclass
            .as_deref()
            .and_then(|sc| self.subclasses.get(sc))
            .and_then(|sc| sc.levels.get(&level));

        for feat in self.features(subclass.as_deref()) {
            let is_new = rules.features.contains(&feat.name)
                || subclass_rules.is_some_and(|r| r.features.contains(&feat.name));
            let already_has = character.features.iter().any(|f| f.name == feat.name);

            // Skip non-stackable features that would be granted again from another source
            if is_new && already_has && !feat.stackable {
                continue;
            }

            if is_new || already_has {
                feat.apply(level, character, &source);
            }
        }

        // Apply hit dice to max HP
        let con_mod = character.ability_modifier(Ability::Constitution);
        let hp_gain = if level == 1 {
            self.hit_die as i32 + con_mod
        } else {
            (self.hit_die as i32) / 2 + 1 + con_mod
        };

        character.combat.hp_max += hp_gain as u32;
        character.combat.hp_current = character.combat.hp_max;
    }
}

// --- Registry ---

struct FetchCache<T: Send + Sync + 'static> {
    data: RwSignal<HashMap<String, T>>,
    pending: RwSignal<HashSet<String>>,
}

impl<T: Send + Sync + 'static> Clone for FetchCache<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Send + Sync + 'static> Copy for FetchCache<T> {}

impl<T: Send + Sync + 'static> std::ops::Deref for FetchCache<T> {
    type Target = RwSignal<HashMap<String, T>>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Send + Sync + 'static> FetchCache<T> {
    fn new() -> Self {
        Self {
            data: RwSignal::new(HashMap::new()),
            pending: RwSignal::new(HashSet::new()),
        }
    }

    fn clear(&self) {
        self.data.update(|m| m.clear());
        self.pending.update(|s| s.clear());
    }
}

impl<T: for<'de> Deserialize<'de> + Send + Sync + 'static> FetchCache<T> {
    /// Fetch a resource if it's not already cached or in-flight.
    /// Returns immediately if the resource is cached or a fetch is pending.
    fn fetch(&self, name: &str, url: String, error_ctx: &'static str) {
        if self.data.read_untracked().contains_key(name) {
            return;
        }
        if self.pending.read_untracked().contains(name) {
            return;
        }

        let name = name.to_string();
        self.pending.update_untracked(|s| s.insert(name.clone()));

        let data = self.data;
        let pending = self.pending;
        leptos::task::spawn_local(async move {
            let result = fetch_json::<T>(&url).await;
            pending.update_untracked(|s| s.remove(&name));
            match result {
                Ok(val) => {
                    data.update(|m| {
                        m.insert(name, val);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch {error_ctx}: {error}");
                }
            }
        });
    }
}

#[derive(Clone, Copy)]
pub struct RulesRegistry {
    locale: Signal<String>,
    class_index: LocalResource<Result<Index, String>>,
    class_cache: FetchCache<ClassDefinition>,
    race_cache: FetchCache<RaceDefinition>,
    background_cache: FetchCache<BackgroundDefinition>,
    spell_list_cache: FetchCache<Vec<SpellDefinition>>,
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

impl RulesRegistry {
    pub fn new(i18n: leptos_fluent::I18n) -> Self {
        let locale = Signal::derive(move || i18n.language.get().id.to_string());

        let class_index = LocalResource::new(move || {
            let locale = locale.get();
            let url = format!("{BASE_URL}/{locale}/index.json");
            async move { fetch_json::<Index>(&url).await }
        });

        let class_cache = FetchCache::new();
        let race_cache = FetchCache::new();
        let background_cache = FetchCache::new();
        let spell_list_cache = FetchCache::new();

        // Clear all caches when locale changes
        let prev_locale = RwSignal::new(locale.get_untracked());
        Effect::new(move || {
            let current = locale.get();
            let prev = prev_locale.get_untracked();
            if current != prev {
                prev_locale.set(current);
                class_cache.clear();
                race_cache.clear();
                background_cache.clear();
                spell_list_cache.clear();
            }
        });

        Self {
            locale,
            class_index,
            class_cache,
            race_cache,
            background_cache,
            spell_list_cache,
        }
    }

    pub fn track_spell_cache(&self) {
        self.spell_list_cache.track();
    }

    fn localized_url(&self, path: &str) -> String {
        let locale = self.locale.get_untracked();
        format!("{BASE_URL}/{locale}/{path}")
    }

    pub fn with_class_entries<R>(&self, f: impl FnOnce(&[ClassIndexEntry]) -> R) -> R {
        let guard = self.class_index.read();
        let entries = guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| idx.classes.as_slice());
        f(entries.unwrap_or(&[]))
    }

    pub fn class_label_by_name(&self, name: &str) -> String {
        self.with_class_entries(|entries| {
            entries
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.label().to_string())
                .unwrap_or_default()
        })
    }

    pub fn has_class(&self, name: &str) -> bool {
        self.class_cache.read_untracked().contains_key(name)
    }

    pub fn with_class<R>(&self, name: &str, f: impl FnOnce(&ClassDefinition) -> R) -> Option<R> {
        self.class_cache.read_untracked().get(name).map(f)
    }

    pub fn with_class_tracked<R>(
        &self,
        name: &str,
        f: impl FnOnce(&ClassDefinition) -> R,
    ) -> Option<R> {
        self.class_cache.read().get(name).map(f)
    }

    pub fn fetch_spell_list(&self, path: &str) {
        let url = self.localized_url(path);
        self.spell_list_cache.fetch(path, url, "spell list");
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
                let cache = self.spell_list_cache.read_untracked();
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
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();

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

    /// Find a feature and call `f` with both the definition and its source.
    pub fn with_feature_source<R>(
        &self,
        identity: &CharacterIdentity,
        feature_name: &str,
        f: impl FnOnce(&FeatureDefinition, FeatureSource) -> R,
    ) -> Option<R> {
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();

        for cl in &identity.classes {
            if let Some(def) = class_cache.get(&cl.class)
                && let Some(feat) = def.find_feature(feature_name, cl.subclass.as_deref())
            {
                return Some(f(feat, FeatureSource::Class(cl.class.clone())));
            }
        }

        if let Some(feat) = bg_cache
            .get(&identity.background)
            .and_then(|def| def.features.get(feature_name))
        {
            return Some(f(
                feat,
                FeatureSource::Background(identity.background.clone()),
            ));
        }

        if let Some(feat) = race_cache
            .get(&identity.race)
            .and_then(|def| def.features.get(feature_name))
        {
            return Some(f(feat, FeatureSource::Race(identity.race.clone())));
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
        let class_cache = self.class_cache.read_untracked();
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
        let cache = self.class_cache.read_untracked();
        for cl in classes {
            if let Some(def) = cache.get(&cl.class)
                && let Some(feat) = def.find_feature(feature_name, cl.subclass.as_deref())
                && let Some(field_def) = feat.fields.get(field_name)
            {
                return Self::resolve_choice_options(field_def, character_fields, cl.level);
            }
        }
        Vec::new()
    }

    fn resolve_choice_options(
        field_def: &FieldDefinition,
        character_fields: &[FeatureField],
        class_level: u32,
    ) -> Vec<ChoiceOption> {
        if let FieldKind::Choice { options, .. } = &field_def.kind {
            return match options {
                ChoiceOptions::List(list) => list
                    .iter()
                    .filter(|o| o.level <= class_level)
                    .cloned()
                    .collect(),
                ChoiceOptions::Ref { from } => character_fields
                    .iter()
                    .find(|cf| cf.name == *from)
                    .into_iter()
                    .flat_map(|cf| cf.value.choices())
                    .filter(|o| !o.name.is_empty())
                    .map(|o| ChoiceOption {
                        name: o.name.clone(),
                        label: o.label.clone(),
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
        let cache = self.class_cache.read_untracked();
        for cl in classes {
            if let Some(def) = cache.get(&cl.class)
                && let Some(feat) = def.find_feature(feature_name, cl.subclass.as_deref())
                && let Some(fd) = feat.fields.get(field_name)
                && let FieldKind::Choice { cost, .. } = &fd.kind
                && let Some(cost_name) = cost
            {
                let short = def
                    .features(cl.subclass.as_deref())
                    .flat_map(|f| f.fields.values())
                    .find(|f| f.name == *cost_name)
                    .and_then(|f| {
                        if let FieldKind::Points { short, .. } = &f.kind {
                            short.as_deref()
                        } else {
                            None
                        }
                    })
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| cost_name.clone());
                return Some(short);
            }
        }
        None
    }

    /// Find the `short` name of a `Points` field by searching across all
    /// class, background, and race features for a field with the given name.
    pub fn get_points_short(
        &self,
        identity: &CharacterIdentity,
        field_name: &str,
    ) -> Option<String> {
        let class_cache = self.class_cache.read_untracked();
        for cl in &identity.classes {
            if let Some(def) = class_cache.get(&cl.class) {
                for feat in def.features(cl.subclass.as_deref()) {
                    if let Some(fd) = feat.fields.get(field_name)
                        && let FieldKind::Points { short: Some(s), .. } = &fd.kind
                    {
                        return Some(s.clone());
                    }
                }
            }
        }

        let bg_cache = self.background_cache.read_untracked();
        if let Some(bg) = bg_cache.get(&identity.background) {
            for feat in bg.features.values() {
                if let Some(fd) = feat.fields.get(field_name)
                    && let FieldKind::Points { short: Some(s), .. } = &fd.kind
                {
                    return Some(s.clone());
                }
            }
        }

        let race_cache = self.race_cache.read_untracked();
        if let Some(race) = race_cache.get(&identity.race) {
            for feat in race.features.values() {
                if let Some(fd) = feat.fields.get(field_name)
                    && let FieldKind::Points { short: Some(s), .. } = &fd.kind
                {
                    return Some(s.clone());
                }
            }
        }

        None
    }

    pub fn fetch_class(&self, name: &str) {
        let url = {
            let guard = self.class_index.read_untracked();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.classes.iter().find(|e| e.name == name) {
                Some(entry) => self.localized_url(&entry.url),
                None => return,
            }
        };

        self.class_cache.fetch(name, url, "class definition");
    }

    pub fn fetch_class_tracked(&self, name: &str) {
        let url = {
            let guard = self.class_index.read();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.classes.iter().find(|e| e.name == name) {
                Some(entry) => self.localized_url(&entry.url),
                None => return,
            }
        };

        self.class_cache.fetch(name, url, "class definition");
    }

    pub fn with_race_entries<R>(&self, f: impl FnOnce(&[RaceIndexEntry]) -> R) -> R {
        let guard = self.class_index.read();
        let entries = guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| idx.races.as_slice());
        f(entries.unwrap_or(&[]))
    }

    pub fn race_label_by_name(&self, name: &str) -> String {
        self.with_race_entries(|entries| {
            entries
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.label().to_string())
                .unwrap_or_default()
        })
    }

    pub fn has_race(&self, name: &str) -> bool {
        self.race_cache.read_untracked().contains_key(name)
    }

    pub fn with_race<R>(&self, name: &str, f: impl FnOnce(&RaceDefinition) -> R) -> Option<R> {
        self.race_cache.read_untracked().get(name).map(f)
    }

    pub fn with_race_tracked<R>(
        &self,
        name: &str,
        f: impl FnOnce(&RaceDefinition) -> R,
    ) -> Option<R> {
        self.race_cache.read().get(name).map(f)
    }

    pub fn fetch_race(&self, name: &str) {
        let url = {
            let guard = self.class_index.read_untracked();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.races.iter().find(|e| e.name == name) {
                Some(entry) => self.localized_url(&entry.url),
                None => return,
            }
        };

        self.race_cache.fetch(name, url, "race definition");
    }

    pub fn fetch_race_tracked(&self, name: &str) {
        let url = {
            let guard = self.class_index.read();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.races.iter().find(|e| e.name == name) {
                Some(entry) => self.localized_url(&entry.url),
                None => return,
            }
        };

        self.race_cache.fetch(name, url, "race definition");
    }

    pub fn with_background_entries<R>(&self, f: impl FnOnce(&[BackgroundIndexEntry]) -> R) -> R {
        let guard = self.class_index.read();
        let entries = guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| idx.backgrounds.as_slice());
        f(entries.unwrap_or(&[]))
    }

    pub fn background_label_by_name(&self, name: &str) -> String {
        self.with_background_entries(|entries| {
            entries
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.label().to_string())
                .unwrap_or_default()
        })
    }

    pub fn has_background(&self, name: &str) -> bool {
        self.background_cache.read_untracked().contains_key(name)
    }

    pub fn with_background<R>(
        &self,
        name: &str,
        f: impl FnOnce(&BackgroundDefinition) -> R,
    ) -> Option<R> {
        self.background_cache.read_untracked().get(name).map(f)
    }

    pub fn with_background_tracked<R>(
        &self,
        name: &str,
        f: impl FnOnce(&BackgroundDefinition) -> R,
    ) -> Option<R> {
        self.background_cache.read().get(name).map(f)
    }

    pub fn fetch_background(&self, name: &str) {
        let url = {
            let guard = self.class_index.read_untracked();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.backgrounds.iter().find(|e| e.name == name) {
                Some(entry) => self.localized_url(&entry.url),
                None => return,
            }
        };

        self.background_cache
            .fetch(name, url, "background definition");
    }

    pub fn fetch_background_tracked(&self, name: &str) {
        let url = {
            let guard = self.class_index.read();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.backgrounds.iter().find(|e| e.name == name) {
                Some(entry) => self.localized_url(&entry.url),
                None => return,
            }
        };

        self.background_cache
            .fetch(name, url, "background definition");
    }

    pub fn with_spell_entries<R>(&self, f: impl FnOnce(&[SpellIndexEntry]) -> R) -> R {
        let guard = self.class_index.read();
        let entries = guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| idx.spells.as_slice());
        f(entries.unwrap_or(&[]))
    }

    pub fn spell_label_by_name(&self, name: &str) -> String {
        self.with_spell_entries(|entries| {
            entries
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.label().to_string())
                .unwrap_or_default()
        })
    }

    pub fn fetch_spell_list_tracked(&self, path: &str) {
        let url = {
            let guard = self.class_index.read();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index
                .spells
                .iter()
                .find(|e| e.url == path || e.name == path)
            {
                Some(entry) => self.localized_url(&entry.url),
                None => {
                    // Fall back to using the path directly (for direct URL references)
                    self.localized_url(path)
                }
            }
        };

        self.spell_list_cache.fetch(path, url, "spell list");
    }

    pub fn with_spell_list_tracked<R>(
        &self,
        path: &str,
        f: impl FnOnce(&[SpellDefinition]) -> R,
    ) -> Option<R> {
        self.spell_list_cache
            .read()
            .get(path)
            .map(|v| f(v.as_slice()))
    }

    /// Fill labels and empty descriptions on a character from registry
    /// definitions. Labels are only filled when `None`. Descriptions are
    /// only filled when empty (preserves user edits). Also triggers
    /// fetches for missing definitions so it self-heals after cache clears.
    pub fn fill_from_registry(&self, character: &mut Character) {
        // Tracked read of the index so the calling Effect re-runs when
        // the index arrives (LocalResource fetch is lazy — this triggers it).
        let index_guard = self.class_index.read();
        let index = index_guard.as_ref().and_then(|r| r.as_ref().ok());

        // Trigger fetches for any missing definitions (needed after locale
        // change clears caches). The fetch methods are no-ops when data is
        // already cached.
        if let Some(idx) = index {
            for cl in &character.identity.classes {
                if !cl.class.is_empty()
                    && let Some(entry) = idx.classes.iter().find(|e| e.name == cl.class)
                {
                    let url = self.localized_url(&entry.url);
                    self.class_cache.fetch(&cl.class, url, "class definition");
                }
            }
            if !character.identity.race.is_empty()
                && let Some(entry) = idx.races.iter().find(|e| e.name == character.identity.race)
            {
                let url = self.localized_url(&entry.url);
                self.race_cache
                    .fetch(&character.identity.race, url, "race definition");
            }
            if !character.identity.background.is_empty()
                && let Some(entry) = idx
                    .backgrounds
                    .iter()
                    .find(|e| e.name == character.identity.background)
            {
                let url = self.localized_url(&entry.url);
                self.background_cache.fetch(
                    &character.identity.background,
                    url,
                    "background definition",
                );
            }
        }

        let class_cache = self.class_cache.read();
        let bg_cache = self.background_cache.read();
        let race_cache = self.race_cache.read();

        // Class/subclass labels (only if empty)
        for cl in &mut character.identity.classes {
            if !cl.class.is_empty()
                && let Some(def) = class_cache.get(&cl.class)
            {
                if cl.class_label.is_none() {
                    cl.class_label = def.label.clone();
                }
                if let Some(sc_name) = &cl.subclass
                    && cl.subclass_label.is_none()
                    && let Some(sc_def) = def.subclasses.get(sc_name)
                {
                    cl.subclass_label = sc_def.label.clone();
                }
            }
        }

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

        // Feature labels and descriptions (only if empty)
        for feature in &mut character.features {
            if !feature.name.is_empty()
                && let Some(feat_def) = find_feat(&feature.name)
            {
                if feature.label.is_none() {
                    feature.label = feat_def.label.clone();
                }
                if feature.description.is_empty() && !feat_def.description.is_empty() {
                    feature.description = feat_def.description.clone();
                }
            }
        }

        // Racial trait labels and descriptions (only if empty)
        if let Some(race_def) = race_cache.get(&character.identity.race) {
            for racial_trait in &mut character.racial_traits {
                if !racial_trait.name.is_empty()
                    && let Some(def_trait) = race_def.traits.get(&racial_trait.name)
                {
                    if racial_trait.label.is_none() {
                        racial_trait.label = def_trait.label.clone();
                    }
                    if racial_trait.description.is_empty() && !def_trait.description.is_empty() {
                        racial_trait.description = def_trait.description.clone();
                    }
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
        let char_level = character.level();

        for (key, entry) in &mut character.feature_data {
            let Some(feat_def) = find_feat(key) else {
                continue;
            };

            // Field labels/descriptions and choice option labels/descriptions (only if
            // empty)
            if !feat_def.fields.is_empty() {
                for field in &mut entry.fields {
                    if let Some(field_def) = feat_def.fields.get(&field.name) {
                        if field.label.is_none() {
                            field.label = field_def.label.clone();
                        }
                        if field.description.is_empty() && !field_def.description.is_empty() {
                            field.description = field_def.description.clone();
                        }

                        if let FieldKind::Choice { options, .. } = &field_def.kind {
                            let def_options = feat_def.resolve_def_options(options);
                            for opt in field.value.choices_mut() {
                                if !opt.name.is_empty()
                                    && let Some(def_opt) =
                                        def_options.iter().find(|o| o.name == opt.name)
                                {
                                    if opt.label.is_none() {
                                        opt.label = def_opt.label.clone();
                                    }
                                    if opt.description.is_empty() && !def_opt.description.is_empty()
                                    {
                                        opt.description = def_opt.description.clone();
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Spell labels, descriptions, cost, and free_uses (only if empty)
            if let Some(spells_def) = &feat_def.spells
                && let Some(spell_data) = &mut entry.spells
            {
                // Resolve FreeUses max from FreeUses field definition
                let free_uses_max = feat_def.fields.values().find_map(|f| match &f.kind {
                    FieldKind::FreeUses { levels } => Some(get_for_level(levels, char_level)),
                    _ => None,
                });

                for spell in &mut spell_data.spells {
                    if !spell.name.is_empty() {
                        let spell_defs: &[SpellDefinition] = match &spells_def.list {
                            SpellList::Inline(spells) => spells,
                            SpellList::Ref { from } => spell_list_cache
                                .get(from.as_str())
                                .map_or(&[], |v| v.as_slice()),
                        };
                        if let Some(def) = spell_defs.iter().find(|s| s.name == spell.name) {
                            if spell.label.is_none() {
                                spell.label = def.label.clone();
                            }
                            if spell.description.is_empty() && !def.description.is_empty() {
                                spell.description = def.description.clone();
                            }
                            spell.cost = def.cost;
                            if let Some(max) = free_uses_max
                                && def.cost > 0
                                && max > 0
                            {
                                match &mut spell.free_uses {
                                    Some(fu) => fu.max = max,
                                    None => {
                                        spell.free_uses = Some(FreeUses { used: 0, max });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Clear labels and descriptions that match registry values.
    /// User-defined content (not matching registry) is preserved.
    pub fn clear_from_registry(&self, character: &mut Character) {
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();

        // Class/subclass labels
        for cl in &mut character.identity.classes {
            if !cl.class.is_empty()
                && let Some(def) = class_cache.get(&cl.class)
            {
                if cl.class_label == def.label {
                    cl.class_label = None;
                }
                if let Some(sc_name) = &cl.subclass
                    && let Some(sc_def) = def.subclasses.get(sc_name)
                    && cl.subclass_label == sc_def.label
                {
                    cl.subclass_label = None;
                }
            }
        }

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

        for feature in &mut character.features {
            if !feature.name.is_empty()
                && let Some(feat_def) = find_feat(&feature.name)
            {
                if feature.label == feat_def.label {
                    feature.label = None;
                }
                if feature.description == feat_def.description {
                    feature.description.clear();
                }
            }
        }

        if let Some(race_def) = race_cache.get(&character.identity.race) {
            for racial_trait in &mut character.racial_traits {
                if !racial_trait.name.is_empty()
                    && let Some(def_trait) = race_def.traits.get(&racial_trait.name)
                {
                    if racial_trait.label == def_trait.label {
                        racial_trait.label = None;
                    }
                    if racial_trait.description == def_trait.description {
                        racial_trait.description.clear();
                    }
                }
            }
        }

        let spell_list_cache = self.spell_list_cache.read_untracked();

        for (key, entry) in &mut character.feature_data {
            let Some(feat_def) = find_feat(key) else {
                continue;
            };

            if !feat_def.fields.is_empty() {
                for field in &mut entry.fields {
                    if let Some(field_def) = feat_def.fields.get(&field.name) {
                        if field.label == field_def.label {
                            field.label = None;
                        }
                        if field.description == field_def.description {
                            field.description.clear();
                        }

                        if let FieldKind::Choice { options, .. } = &field_def.kind {
                            let def_options = feat_def.resolve_def_options(options);
                            for opt in field.value.choices_mut() {
                                if !opt.name.is_empty()
                                    && let Some(def_opt) =
                                        def_options.iter().find(|o| o.name == opt.name)
                                {
                                    if opt.label == def_opt.label {
                                        opt.label = None;
                                    }
                                    if opt.description == def_opt.description {
                                        opt.description.clear();
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(spells_def) = &feat_def.spells
                && let Some(spell_data) = &mut entry.spells
            {
                for spell in &mut spell_data.spells {
                    if !spell.name.is_empty() {
                        let spell_defs: &[SpellDefinition] = match &spells_def.list {
                            SpellList::Inline(spells) => spells,
                            SpellList::Ref { from } => spell_list_cache
                                .get(from.as_str())
                                .map_or(&[], |v| v.as_slice()),
                        };
                        if let Some(def) = spell_defs.iter().find(|s| s.name == spell.name) {
                            if spell.label == def.label {
                                spell.label = None;
                            }
                            if spell.description == def.description {
                                spell.description.clear();
                            }
                        }
                    }
                    spell.cost = 0;
                    spell.free_uses = None;
                }
            }
        }
    }
}
