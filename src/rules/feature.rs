use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Deserializer, de};

use super::spells::SpellsDefinition;
use crate::{
    demap::{self, Named},
    expr::{self, Expr},
    model::{
        Armor, ArmorType, Attribute, Character, Context, Die, Feature, FeatureField, FeatureSource,
        FeatureValue, Translatable,
    },
    rules::utils::get_for_level,
    vecset::VecSet,
};

/// A field value that is either a static number or an expression evaluated
/// against the character (e.g. `"max(1, CHA.MOD)"` for Bardic Inspiration
/// uses).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ValueOrExpr {
    Value(u32),
    Expr(Expr<Attribute>),
}

impl Default for ValueOrExpr {
    fn default() -> Self {
        Self::Value(0)
    }
}

impl ValueOrExpr {
    pub fn eval(&self, ctx: &impl expr::Context<Attribute>) -> u32 {
        match self {
            Self::Value(v) => *v,
            Self::Expr(expr) => expr.eval(ctx).unwrap_or(0).max(0) as u32,
        }
    }
}

/// A die pool definition that accepts either a static die string (`"2d6"`)
/// or an object with expression-based amount (`{"sides": 6, "amount":
/// "CHA.MOD"}`).
#[derive(Debug, Clone)]
pub struct DieOrExpr {
    pub sides: u32,
    pub amount: ValueOrExpr,
}

impl Default for DieOrExpr {
    fn default() -> Self {
        Self {
            sides: 0,
            amount: ValueOrExpr::Value(0),
        }
    }
}

impl DieOrExpr {
    pub fn eval(&self, ctx: &impl expr::Context<Attribute>) -> Die {
        Die {
            amount: self.amount.eval(ctx),
            sides: self.sides,
        }
    }
}

impl<'de> Deserialize<'de> for DieOrExpr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DieOrExprVisitor;

        impl<'de> de::Visitor<'de> for DieOrExprVisitor {
            type Value = DieOrExpr;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a die string like \"2d6\" or an object {sides, amount}")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<DieOrExpr, E> {
                let die: Die = s.parse().map_err(de::Error::custom)?;
                Ok(DieOrExpr {
                    sides: die.sides,
                    amount: ValueOrExpr::Value(die.amount),
                })
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<DieOrExpr, A::Error> {
                #[derive(Deserialize)]
                struct Fields {
                    sides: u32,
                    amount: ValueOrExpr,
                }
                let f = Fields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(DieOrExpr {
                    sides: f.sides,
                    amount: f.amount,
                })
            }
        }

        deserializer.deserialize_any(DieOrExprVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ActionType {
    Action,
    BonusAction,
    Reaction,
}

impl ActionType {
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Action => "swords",
            Self::BonusAction => "zap",
            Self::Reaction => "shield",
        }
    }
}

impl Translatable for ActionType {
    fn tr_key(&self) -> &'static str {
        match self {
            Self::Action => "action-type-action",
            Self::BonusAction => "action-type-bonus-action",
            Self::Reaction => "action-type-reaction",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeatureDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
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
    #[serde(default)]
    pub ac_expr: Option<Expr<Attribute>>,
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
    OnCompute,
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

        // Create Natural armor entry if feature defines an AC expression.
        // ac_expr is level-independent, so we only insert once and skip on re-apply.
        if let Some(ac_expr) = &self.ac_expr {
            let already_exists = character
                .equipment
                .armors
                .iter()
                .any(|a| a.armor_type == ArmorType::Natural && a.name == self.name);
            if !already_exists {
                character.equipment.armors.push(Armor {
                    name: self.name.clone(),
                    armor_type: ArmorType::Natural,
                    ac_expr: Some(ac_expr.clone()),
                    ..Default::default()
                });
            }
        }
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

        let is_new = character
            .feature_data
            .get(&self.name)
            .is_none_or(|e| e.fields.is_empty());

        if is_new {
            // Pre-compute values before mutating feature_data
            let new_fields: Vec<_> = self
                .fields
                .values()
                .filter(|field_def| !matches!(field_def.kind, FieldKind::FreeUses { .. }))
                .map(|field_def| FeatureField {
                    name: field_def.name.clone(),
                    label: field_def.label.clone(),
                    description: field_def.description.clone(),
                    value: field_def.kind.to_value(level, character),
                })
                .collect();
            let entry = character.feature_data.entry(self.name.clone()).or_default();
            if entry.source.is_none() {
                entry.source = Some(source.clone());
            }
            entry.fields = new_fields;
        } else {
            // Pre-compute expression-based values (needs &character before mutation)
            let evaluated: Vec<_> = character
                .feature_data
                .get(&self.name)
                .into_iter()
                .flat_map(|e| e.fields.iter())
                .filter_map(|field| {
                    let def = self.fields.get(field.name.as_str())?;
                    match &def.kind {
                        FieldKind::Points { .. } | FieldKind::Die { .. } => {
                            Some((field.name.clone(), def.kind.to_value(level, character)))
                        }
                        _ => None,
                    }
                })
                .collect();

            let entry = character.feature_data.entry(self.name.clone()).or_default();
            if entry.source.is_none() {
                entry.source = Some(source.clone());
            }
            for field in entry.fields.iter_mut() {
                if let Some(def) = self.fields.get(field.name.as_str()) {
                    match (&def.kind, &mut field.value) {
                        (FieldKind::Die { .. }, FeatureValue::Die { die, .. }) => {
                            if let Some((_, FeatureValue::Die { die: new_die, .. })) =
                                evaluated.iter().find(|(n, _)| n == &field.name)
                            {
                                *die = *new_die;
                            }
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
                        (FieldKind::Points { .. }, FeatureValue::Points { max, .. }) => {
                            if let Some((_, FeatureValue::Points { max: new_max, .. })) =
                                evaluated.iter().find(|(n, _)| n == &field.name)
                            {
                                *max = *new_max;
                            }
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
                    action: None,
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
        levels: BTreeMap<u32, ValueOrExpr>,
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
        levels: BTreeMap<u32, DieOrExpr>,
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
    pub fn has_levels(&self) -> bool {
        match self {
            Self::Points { levels, .. } => !levels.is_empty(),
            Self::Choice { levels, .. } => !levels.is_empty(),
            Self::Die { levels } => !levels.is_empty(),
            Self::Bonus { levels } => !levels.is_empty(),
            Self::FreeUses { levels } => !levels.is_empty(),
        }
    }

    pub fn to_value(&self, level: u32, character: &Character) -> FeatureValue {
        match self {
            Self::Die { levels } => FeatureValue::Die {
                die: get_for_level(levels, level).eval(character),
                used: 0,
            },
            Self::Choice { levels, .. } => FeatureValue::Choice {
                options: vec![Default::default(); get_for_level(levels, level) as usize],
            },
            Self::Bonus { levels } => FeatureValue::Bonus(get_for_level(levels, level)),
            Self::Points { levels, .. } => FeatureValue::Points {
                used: 0,
                max: get_for_level(levels, level).eval(character),
            },
            Self::FreeUses { .. } => {
                unreachable!("FreeUses fields are not converted to FeatureValue")
            }
        }
    }

    /// Re-evaluate dynamic field values (expressions that depend on
    /// character state like CHA.MOD). Returns a new `FeatureValue` if the
    /// field has expression-based values that need updating.
    pub fn recompute_dynamic(&self, level: u32, character: &Character) -> Option<FeatureValue> {
        match self {
            Self::Points { levels, .. } => {
                let value = get_for_level(levels, level);
                matches!(value, ValueOrExpr::Expr(_)).then(|| self.to_value(level, character))
            }
            Self::Die { levels } => {
                let value = get_for_level(levels, level);
                matches!(value.amount, ValueOrExpr::Expr(_))
                    .then(|| self.to_value(level, character))
            }
            _ => None,
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
    #[serde(default)]
    pub action: Option<ActionType>,
}

impl ChoiceOption {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}
