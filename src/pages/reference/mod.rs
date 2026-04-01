pub mod background;
pub mod class;
pub mod feature;
pub mod sidebar;
pub mod species;
pub mod spell;
use std::collections::BTreeMap;

use leptos::{either::EitherOf3, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::components::A;
pub use sidebar::ReferenceSidebar;

/// Percent-encode a name for use in URL paths (spaces, parens, etc.).
pub fn encode_name(name: &str) -> String {
    js_sys::encode_uri_component(name).into()
}

use crate::{
    BASE_URL,
    components::expr_view::ExprView,
    expr::{BLOCK_ERROR, BLOCK_NOOP, Expr, Interpreter, Op},
    model::{Attribute, Translatable},
    rules::{Assignment, ChoiceOptions, FeatureDefinition, FieldDefinition, FieldKind, SpellList},
};

pub struct InlineSpell {
    pub label: String,
    pub level: u32,
    pub min_level: u32,
    pub sticky: bool,
    pub description: String,
    pub effects: Vec<(String, Expr<Attribute>)>,
}

pub enum FeatureSpells {
    None,
    Link(String),
    Inline(Vec<InlineSpell>),
}

impl FeatureSpells {
    pub fn from_spell_list(list: Option<&SpellList>) -> Self {
        match list {
            Some(spell_list @ SpellList::Ref { from }) => {
                let list_name = spell_list.ref_name().unwrap_or(from);
                Self::Link(list_name.to_string())
            }
            Some(SpellList::Inline(spells)) if !spells.is_empty() => Self::Inline(
                spells
                    .values()
                    .map(|s| InlineSpell {
                        label: s.label().to_string(),
                        level: s.level,
                        min_level: s.min_level,
                        sticky: s.sticky,
                        description: s.description.clone(),
                        effects: s
                            .effects
                            .iter()
                            .map(|e| (e.label().to_string(), e.expr.clone()))
                            .collect(),
                    })
                    .collect(),
            ),
            _ => Self::None,
        }
    }
}

pub struct InlineChoiceOption {
    pub label: String,
    pub level: u32,
    pub cost: u32,
    pub description: String,
    pub effects: Vec<(String, Expr<Attribute>)>,
}

pub struct ChoiceFieldView {
    pub label: String,
    pub description: String,
    pub cost_unit: Option<String>,
    pub options: Vec<InlineChoiceOption>,
}

pub fn feature_choices(
    fields: &BTreeMap<Box<str>, FieldDefinition>,
) -> Option<Vec<ChoiceFieldView>> {
    let values: Vec<_> = fields
        .values()
        .filter_map(|fd| {
            let FieldKind::Choice {
                options: ChoiceOptions::List(list),
                cost,
                ..
            } = &fd.kind
            else {
                return None;
            };
            if list.is_empty() {
                return None;
            }
            Some(ChoiceFieldView {
                label: fd.label().to_string(),
                description: fd.description.clone(),
                cost_unit: cost.clone(),
                options: list
                    .iter()
                    .map(|opt| InlineChoiceOption {
                        label: opt.label().to_string(),
                        level: opt.level,
                        cost: opt.cost,
                        description: opt.description.clone(),
                        effects: opt
                            .effects
                            .iter()
                            .map(|e| (e.label().to_string(), e.expr.clone()))
                            .collect(),
                    })
                    .collect(),
            })
        })
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// Translate an attribute to a human-readable display name using i18n keys.
/// An interpreter that produces human-readable translated summaries
/// of assignment operations in an expression. Categorizes by type
/// (abilities, skill/save/equipment proficiencies, other effects).
/// Stack entry: display string + optional numeric value for arithmetic.
/// Stack entry: display string, optional numeric value, and optional
/// "compound base" variable (tracks `X` in `X + expr` for compound
/// assignment detection).
struct SumEntry {
    text: String,
    num: Option<i32>,
    /// Raw attribute key for compound detection (e.g. "INITIATIVE.BONUS").
    raw_key: Option<String>,
    /// If this entry is `var op rhs`, stores `(raw_var_key, op, rhs_text)`.
    compound: Option<(String, String, String)>,
}

impl SumEntry {
    fn constant(n: i32) -> Self {
        Self {
            text: n.to_string(),
            num: Some(n),
            raw_key: None,
            compound: None,
        }
    }

    fn var(text: String, raw_key: String) -> Self {
        Self {
            text,
            num: None,
            raw_key: Some(raw_key),
            compound: None,
        }
    }
}

struct AssignmentSummarizer<'a> {
    stack: Vec<SumEntry>,
    i18n: &'a leptos_fluent::I18n,
    abilities: Vec<String>,
    skills: Vec<String>,
    saves: Vec<String>,
    equipment: Vec<String>,
    other: Vec<String>,
}

impl<'a> AssignmentSummarizer<'a> {
    fn new(i18n: &'a leptos_fluent::I18n) -> Self {
        Self {
            stack: Vec::new(),
            i18n,
            abilities: Vec::new(),
            skills: Vec::new(),
            saves: Vec::new(),
            equipment: Vec::new(),
            other: Vec::new(),
        }
    }

    fn pop(&mut self) -> SumEntry {
        self.stack.pop().unwrap_or(SumEntry::constant(0))
    }

    fn binary_op(&mut self, op_str: &str, f: impl FnOnce(i32, i32) -> i32) {
        let b = self.pop();
        let a = self.pop();
        let num = a.num.zip(b.num).map(|(a, b)| f(a, b));
        let text = num.map_or_else(
            || format!("{} {} {}", a.text, op_str, b.text),
            |n| n.to_string(),
        );
        // Track compound: if `a` is a plain variable, record raw key + op + rhs
        let compound = if let (Some(key), None) = (a.raw_key, &a.compound) {
            Some((key, op_str.to_string(), b.text))
        } else {
            None
        };
        self.stack.push(SumEntry {
            text,
            num,
            raw_key: None,
            compound,
        });
    }
}

impl Interpreter<Attribute, i32> for AssignmentSummarizer<'_> {
    type Output = String;

    fn exec(
        &mut self,
        op: Op<Attribute, i32>,
    ) -> Result<Option<crate::expr::BlockIndex>, crate::expr::Error> {
        match op {
            Op::PushConst(n) => self.stack.push(SumEntry::constant(n)),
            Op::PushVar(var) => {
                let raw = var.to_string();
                let text = var.display_name(self.i18n);
                self.stack.push(SumEntry::var(text, raw));
            }
            Op::Add => self.binary_op("+", |a, b| a + b),
            Op::Sub => self.binary_op("-", |a, b| a - b),
            Op::Mul => self.binary_op("*", |a, b| a * b),
            Op::DivFloor => self.binary_op("/", |a, b| if b != 0 { a / b } else { 0 }),
            Op::DivCeil => {
                self.binary_op("\\", |a, b| if b != 0 { (a + b - 1) / b } else { 0 });
            }
            Op::Mod => self.binary_op("%", |a, b| if b != 0 { a % b } else { 0 }),
            Op::Min => self.binary_op("min", |a, b| a.min(b)),
            Op::Max => self.binary_op("max", |a, b| a.max(b)),
            Op::AvgHp => {
                let a = self.pop();
                let num = a.num.map(crate::expr::avg_hp);
                let text = num.map_or_else(|| format!("avg_hp({})", a.text), |n| n.to_string());
                self.stack.push(SumEntry {
                    text,
                    num,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::Assign(attr) => {
                let value = self.pop();
                let attr_str = attr.to_string();
                // Detect compound: X op= expr → show "op rhs", otherwise just "value"
                let (prefix, display) = if let Some((base, op, rhs)) = &value.compound {
                    if *base == attr_str {
                        (op.as_str(), rhs.clone())
                    } else {
                        ("", value.text.clone())
                    }
                } else {
                    ("", value.text.clone())
                };
                match attr {
                    Attribute::Ability(ability) => {
                        let label = self.i18n.tr(ability.tr_abbr_key());
                        self.abilities.push(format!("{label} {prefix}{display}"));
                    }
                    Attribute::SkillProficiency(skill) => {
                        self.skills.push(self.i18n.tr(skill.tr_key()));
                    }
                    Attribute::SaveProficiency(ability) => {
                        self.saves.push(self.i18n.tr(ability.tr_abbr_key()));
                    }
                    Attribute::EquipmentProficiency(prof) => {
                        self.equipment.push(self.i18n.tr(prof.tr_key()));
                    }
                    _ => {
                        let label = attr.display_name(self.i18n);
                        self.other.push(format!("{label} {prefix}{display}"));
                    }
                }
            }
            Op::Cmp(cmp) => {
                let b = self.pop();
                let a = self.pop();
                let sym = match cmp.symbol() {
                    "<=" => "≤",
                    ">=" => "≥",
                    "!=" => "≠",
                    "==" => "=",
                    s => s,
                };
                self.stack.push(SumEntry {
                    text: format!("{} {sym} {}", a.text, b.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::And => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(SumEntry {
                    text: format!("{} and {}", a.text, b.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::Or => {
                let b = self.pop();
                let a = self.pop();
                self.stack.push(SumEntry {
                    text: format!("{} or {}", a.text, b.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::Not => {
                let a = self.pop();
                self.stack.push(SumEntry {
                    text: format!("not {}", a.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::In => {
                let c = self.pop();
                let b = self.pop();
                let a = self.pop();
                self.stack.push(SumEntry {
                    text: format!("{} ≤ {} ≤ {}", b.text, a.text, c.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            // if(): evaluate both branches to collect all possible assignments
            Op::EvalIf(then_idx, else_idx) => {
                self.pop(); // condition
                // Return first non-noop block; the runner will recurse into it.
                if then_idx != BLOCK_NOOP && then_idx != BLOCK_ERROR {
                    return Ok(Some(then_idx));
                }
                if else_idx != BLOCK_NOOP && else_idx != BLOCK_ERROR {
                    return Ok(Some(else_idx));
                }
            }
            Op::Eval(idx) => {
                if idx != BLOCK_NOOP {
                    return Ok(Some(idx));
                }
            }
            // Dice/roll ops: push a placeholder
            Op::Roll | Op::Sum | Op::Explode => {
                self.pop();
                self.pop();
                self.stack.push(SumEntry::constant(0));
            }
            Op::KeepMax(_) | Op::KeepMin(_) | Op::DropMax(_) | Op::DropMin(_) => {
                // Modifier on a roll result — replace top
                let top = self.pop();
                self.stack.push(top);
            }
        }
        Ok(None)
    }

    fn finish(self) -> Result<Self::Output, crate::expr::Error> {
        let mut parts = Vec::new();
        if !self.abilities.is_empty() {
            parts.push(self.abilities.join(", "));
        }
        if !self.skills.is_empty() {
            parts.push(self.skills.join(", "));
        }
        if !self.saves.is_empty() {
            parts.push(self.saves.join(", "));
        }
        if !self.equipment.is_empty() {
            parts.push(self.equipment.join(", "));
        }
        parts.extend(self.other);
        // Remaining stack entries (e.g. prerequisites — boolean expressions)
        for entry in self.stack {
            if entry.num.is_none() && !entry.text.is_empty() {
                parts.push(entry.text);
            }
        }
        Ok(parts.join(" | "))
    }
}

/// Extract human-readable assignment summaries from feature expressions.
pub(super) fn summarize_assignments(
    assignments: &[Assignment],
    i18n: &leptos_fluent::I18n,
) -> String {
    assignments
        .iter()
        .filter_map(|a| a.expr.run(AssignmentSummarizer::new(i18n)).ok())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Pre-collected data for rendering a feature in reference pages.
pub struct FeatureViewData {
    pub name: String,
    pub label: String,
    pub category: String,
    pub description: String,
    pub languages: String,
    pub prerequisites: String,
    pub assignments: String,
    pub spells: FeatureSpells,
    pub choices: Option<Vec<ChoiceFieldView>>,
}

/// Collect feature view data from an iterator of `FeatureDefinition`
/// references.
pub fn collect_feature_views<'a>(
    features: impl Iterator<Item = &'a FeatureDefinition>,
) -> Vec<FeatureViewData> {
    let i18n = expect_context::<leptos_fluent::I18n>();
    features
        .map(|feat| {
            let prerequisites = feat
                .prerequisites
                .as_ref()
                .and_then(|p| p.run(AssignmentSummarizer::new(&i18n)).ok())
                .unwrap_or_default();
            let assignments = feat
                .assign
                .as_deref()
                .map(|a| summarize_assignments(a, &i18n))
                .unwrap_or_default();
            FeatureViewData {
                name: feat.name.clone(),
                label: feat.label().to_string(),
                category: i18n.tr(feat.category.tr_key()),
                description: feat.description.clone(),
                languages: feat.languages.join(", "),
                prerequisites,
                assignments,
                spells: FeatureSpells::from_spell_list(
                    feat.spells.as_ref().map(|spells_def| &spells_def.list),
                ),
                choices: feature_choices(&feat.fields),
            }
        })
        .collect()
}

/// Render a list of reference features.
#[component]
pub fn ReferenceFeaturesView(
    features: Vec<FeatureViewData>,
    #[prop(optional)] anchors: bool,
) -> impl IntoView {
    if features.is_empty() {
        return None;
    }
    Some(view! {
        <div class="reference-features">
            {features
                .into_iter()
                .map(|feat| {
                    let id = anchors.then(|| format!("feat-{}", feat.name));
                    view! {
                        <div class="reference-feature" id=id>
                            <h3>{feat.label}</h3>
                            <p class="feature-prerequisites">
                                {feat.category}
                                {(!feat.prerequisites.is_empty()).then(|| view! {
                                    {" · "}{move_tr!("ref-prerequisites")}{": "}{feat.prerequisites}
                                })}
                            </p>
                            <p>{feat.description}</p>
                            {(!feat.languages.is_empty()).then(|| view! {
                                <p class="feature-languages">
                                    {move_tr!("ref-languages")}{": "}{feat.languages}
                                </p>
                            })}
                            {(!feat.assignments.is_empty()).then(|| view! {
                                <p class="feature-assignments">{feat.assignments}</p>
                            })}
                            <FeatureSpellsView spells=feat.spells />
                            <FeatureChoicesView choices=feat.choices />
                        </div>
                    }
                })
                .collect_view()}
        </div>
    })
}

#[component]
pub fn FeatureChoicesView(choices: Option<Vec<ChoiceFieldView>>) -> impl IntoView {
    choices.map(|fields| {
        view! {
            <div class="feature-choices-inline">
                {fields
                    .into_iter()
                    .map(|field| {
                        let label = field.label;
                        let desc = field.description;
                        let cost_unit = field.cost_unit;
                        let options = field.options;
                        view! {
                            <div class="feature-choice-field">
                                <strong>{label}</strong>
                                {(!desc.is_empty()).then(|| view! { <p>{desc}</p> })}
                                <div class="feature-choice-options">
                                    {options
                                        .into_iter()
                                        .map(|opt| {
                                            let level = opt.level;
                                            let cost = opt.cost;
                                            let unit = cost_unit.clone();
                                            let opt_label = opt.label;
                                            let opt_desc = opt.description;
                                            view! {
                                                <div class="feature-choice-entry">
                                                    <strong>{opt_label}</strong>
                                                    {(level > 0 || (cost > 0 && unit.is_some()))
                                                        .then(|| {
                                                            view! {
                                                                {" ("}
                                                                {(level > 0).then(|| {
                                                                    view! {
                                                                        {move_tr!(
                                                                            "ref-spell-min-level",
                                                                            { "level" => level
                                                                            .to_string() }
                                                                        )}
                                                                    }
                                                                })}
                                                                {(cost > 0).then(|| {
                                                                    let u = unit
                                                                        .clone()
                                                                        .unwrap_or_default();
                                                                    let sep = if level > 0 {
                                                                        ", "
                                                                    } else {
                                                                        ""
                                                                    };
                                                                    view! {
                                                                        {sep}
                                                                        {cost.to_string()}
                                                                        {" "}
                                                                        {u}
                                                                    }
                                                                })}
                                                                {")"}
                                                            }
                                                        })}
                                                    {(!opt_desc.is_empty())
                                                        .then(|| view! { <p>{opt_desc}</p> })}
                                                    {(!opt.effects.is_empty()).then(|| view! {
                                                        <div class="spell-effects">
                                                            {opt.effects.into_iter().map(|(name, expr)| view! {
                                                                <div class="spell-effect">
                                                                    <strong>{name}</strong>
                                                                    <ExprView expr />
                                                                </div>
                                                            }).collect_view()}
                                                        </div>
                                                    })}
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        }
    })
}

#[component]
pub fn FeatureSpellsView(spells: FeatureSpells) -> impl IntoView {
    match spells {
        FeatureSpells::Link(list_name) => EitherOf3::A(view! {
            <p class="feature-spell-link">
                <A href=format!("{BASE_URL}/r/spell/{list_name}")>
                    {move_tr!("ref-spell-list-link")}
                </A>
            </p>
        }),
        FeatureSpells::Inline(spells) => EitherOf3::B(view! {
            <div class="feature-spells-inline">
                {spells.into_iter().map(|spell| {
                    let level_text = if spell.level == 0 {
                        move_tr!("ref-cantrips-level")
                    } else {
                        move_tr!("ref-spell-level", {"level" => spell.level})
                    };
                    let min_level = spell.min_level;
                    let sticky = spell.sticky;
                    view! {
                        <div class="feature-spell-entry">
                            <strong>{spell.label}</strong>
                            {" ("}{level_text}
                            {sticky.then(|| view! {
                                {", "}{move_tr!("ref-spell-always-ready")}
                            })}
                            {(min_level > 0).then(|| view! {
                                {", "}{move_tr!("ref-spell-min-level", {"level" => min_level})}
                            })}
                            {")"}
                            {(!spell.description.is_empty()).then(|| view! {
                                <p>{spell.description}</p>
                            })}
                            {(!spell.effects.is_empty()).then(|| view! {
                                <div class="spell-effects">
                                    {spell.effects.into_iter().map(|(name, expr)| view! {
                                        <div class="spell-effect">
                                            <strong>{name}</strong>
                                            <ExprView expr />
                                        </div>
                                    }).collect_view()}
                                </div>
                            })}
                        </div>
                    }
                }).collect_view()}
            </div>
        }),
        FeatureSpells::None => EitherOf3::C(()),
    }
}
