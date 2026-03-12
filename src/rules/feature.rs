use std::collections::BTreeMap;

use serde::Deserialize;

use super::spells::SpellsDefinition;
use crate::{
    demap::{self, Named},
    expr::Expr,
    model::{
        Attribute, Character, Context, Die, Feature, FeatureField, FeatureSource, FeatureValue,
    },
    rules::utils::get_for_level,
    vecset::VecSet,
};

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
    pub fields: BTreeMap<Box<str>, FieldDefinition>,
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
    pub fn resolve_def_options<'a>(&'a self, options: &'a ChoiceOptions) -> &'a [ChoiceOption] {
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

    pub fn assign<'a>(&self, mut context: Context<'a>, when: WhenCondition) {
        log::info!(
            "Checking assignments for feature '{}', when condition: {when:?}",
            self.name,
        );

        let Some(assign) = &self.assign else { return };

        assign.iter().filter(|a| a.when == when).for_each(|a| {
            log::info!(
                "Applying assignment for feature '{}': {:?} (when: {:?})",
                self.name,
                a.expr,
                a.when,
            );
            match a.expr.apply(&mut context) {
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
        let when = if character.features.iter().any(|f| f.name == self.name) {
            WhenCondition::OnLevelUp
        } else {
            character.features.push(Feature {
                name: self.name.clone(),
                label: self.label.clone(),
                description: self.description.clone(),
            });
            WhenCondition::OnFeatureAdd
        };

        character.languages.extend(self.languages.iter().cloned());

        let (caster_level, caster_modifier) = if let Some(spells_def) = &self.spells {
            let free_uses_max = self.free_uses_max(level);
            spells_def.apply(level, character, &self.name, source, free_uses_max);
            (
                character.caster_level(spells_def.pool) as i32,
                character.ability_modifier(spells_def.casting_ability),
            )
        } else {
            (0, 0)
        };

        self.assign(
            Context {
                character,
                class_level: level as i32,
                caster_level,
                caster_modifier,
            },
            when,
        );

        self.apply_fields(level, character, source);
    }

    fn free_uses_max(&self, level: u32) -> u32 {
        self.fields
            .values()
            .find_map(|field_def| match &field_def.kind {
                FieldKind::FreeUses { levels } => Some(get_for_level(levels, level)),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn apply_fields(&self, level: u32, character: &mut Character, source: &FeatureSource) {
        if self.fields.is_empty() {
            return;
        }

        let entry = character.feature_data.entry(self.name.clone()).or_default();
        if entry.source.is_none() {
            entry.source = Some(source.clone());
        }
        let fields = &mut entry.fields;
        if fields.is_empty() {
            *fields = self
                .fields
                .values()
                .filter(|field_def| !matches!(field_def.kind, FieldKind::FreeUses { .. }))
                .map(|field_def| FeatureField {
                    name: field_def.name.clone(),
                    label: field_def.label.clone(),
                    description: field_def.description.clone(),
                    value: field_def.kind.to_value(level),
                })
                .collect();
        } else {
            for field in fields.iter_mut() {
                if let Some(def) = self.fields.get(field.name.as_str()) {
                    match (&def.kind, &mut field.value) {
                        (FieldKind::Die { levels }, FeatureValue::Die { die, .. }) => {
                            *die = get_for_level(levels, level);
                        }
                        (FieldKind::Choice { levels, .. }, FeatureValue::Choice { options }) => {
                            let new_len = get_for_level(levels, level) as usize;
                            if options.len() < new_len {
                                options.resize(new_len, Default::default());
                            }
                        }
                        (FieldKind::Bonus { levels }, FeatureValue::Bonus(b)) => {
                            *b = get_for_level(levels, level);
                        }
                        (FieldKind::Points { levels, .. }, FeatureValue::Points { max, .. }) => {
                            *max = get_for_level(levels, level);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
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

    pub fn resolve_choice_options(
        &self,
        character_fields: &[FeatureField],
        class_level: u32,
    ) -> Vec<ChoiceOption> {
        let FieldKind::Choice { options, .. } = &self.kind else {
            return Vec::new();
        };

        match options {
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
        }
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
            Self::Die { levels } => FeatureValue::Die {
                die: get_for_level(levels, level),
                used: 0,
            },
            Self::Choice { levels, .. } => FeatureValue::Choice {
                options: vec![Default::default(); get_for_level(levels, level) as usize],
            },
            Self::Bonus { levels } => FeatureValue::Bonus(get_for_level(levels, level)),
            Self::Points { levels, .. } => FeatureValue::Points {
                used: 0,
                max: get_for_level(levels, level),
            },
            Self::FreeUses { .. } => {
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
