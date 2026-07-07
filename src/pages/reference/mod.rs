pub mod background;
pub mod class;
pub mod feature;
pub mod progression;
pub mod sidebar;
pub mod species;
pub mod spell;

use std::collections::BTreeMap;

use leptos::{either::EitherOf3, prelude::*};
use leptos_fluent::{I18n, move_tr};
pub use sidebar::{RefSidebarEntries, ReferenceSidebar};

use crate::{
    components::{
        expr_view::ExprView, markdown::Markdown, ref_link::Ref, spell_info_bar::SpellInfoBar,
    },
    expr::{
        self, BLOCK_ERROR, BLOCK_NOOP, BinOp, BlockIndex, CursorStack, GroupCursor, Interpreter,
    },
    model::{Attribute, AttributeGroup, Expr, FeatureCategory, Op, StaticAttrSource, Translatable},
    rules::{
        ActionDefinition, Assignment, ChoiceOptions, FeatureDefinition, PoolSummarizer,
        RulesRegistry, SpellDefinition, SpellEntry, SpellsList, locale::LocaleKey,
    },
};

/// Percent-encode a name for use in URL paths (spaces, parens, etc.).
pub fn encode_name(name: &str) -> String {
    js_sys::encode_uri_component(name).into()
}

/// One row in `FeatureSpells::Inline` — the per-feature override
/// (`SpellEntry`) plus the spell name. The `SpellDefinition` and
/// locale text are resolved by the row view from the registry on
/// render, so locale switches only patch text instead of rebuilding
/// the row.
#[derive(Clone)]
pub struct InlineSpellKey {
    pub name: String,
    pub entry: SpellEntry,
}

#[derive(Clone)]
pub enum FeatureSpells {
    None,
    Link(String),
    Inline(Vec<InlineSpellKey>),
}

impl FeatureSpells {
    pub fn from_feature(feat_def: &FeatureDefinition) -> Self {
        let list = feat_def.spells.as_ref().map(|spells_def| &spells_def.list);
        // Sticky grants are expressed as `STICKY.<name> = 1` assigns; recover
        // them so the reference page still shows prepared spells. min_level
        // is back-derived by evaluating OnCompute assigns at LEVEL 1..=20 and
        // capturing the first level where each Sticky var becomes 1.
        let sticky_grants = PoolSummarizer::new(feat_def).sticky_grants();

        if let Some(spell_list @ SpellsList::Ref { from }) = list {
            let list_name = spell_list.ref_name().unwrap_or(from);
            return Self::Link(list_name.to_string());
        }

        let mut rows: Vec<InlineSpellKey> = Vec::new();
        for grant in &sticky_grants {
            rows.push(InlineSpellKey {
                name: grant.name.to_string(),
                entry: SpellEntry {
                    name: grant.name.into(),
                    sticky: true,
                    min_level: grant.min_level,
                    cost: 0,
                },
            });
        }
        if let Some(SpellsList::Inline(entries)) = list {
            for entry in entries {
                if rows.iter().any(|row| row.name == *entry.name) {
                    continue;
                }
                rows.push(InlineSpellKey {
                    name: entry.name.to_string(),
                    entry: entry.clone(),
                });
            }
        }
        if rows.is_empty() {
            Self::None
        } else {
            Self::Inline(rows)
        }
    }
}

/// Stable identity for one Choice option on a feature action. Locale
/// text + structural fields (level/cost/effects) are resolved at
/// render time from the feature's `ActionDefinition`.
#[derive(Clone)]
pub struct ChoiceOptionKey {
    pub feat_name: String,
    pub field_name: String,
    pub name: String,
}

#[derive(Clone)]
pub struct ChoiceFieldKey {
    pub feat_name: String,
    pub field_name: String,
    pub options: Vec<ChoiceOptionKey>,
}

#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "ref.feature_choices", skip(actions), fields(feat = feat_name))
)]
fn feature_choices(
    feat_name: &str,
    actions: &BTreeMap<Box<str>, ActionDefinition>,
) -> Option<Vec<ChoiceFieldKey>> {
    let values: Vec<_> = actions
        .values()
        .filter_map(|action_def| {
            let ChoiceOptions::List(list) = &action_def.options else {
                return None;
            };
            if list.is_empty() {
                return None;
            }
            Some(ChoiceFieldKey {
                feat_name: feat_name.to_string(),
                field_name: action_def.name.to_string(),
                options: list
                    .iter()
                    .map(|opt| ChoiceOptionKey {
                        feat_name: feat_name.to_string(),
                        field_name: action_def.name.to_string(),
                        name: opt.name.to_string(),
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

/// Extract `(label, expr)` pairs for a spell's effects (snapshot — `ExprView`
/// renders the raw expression). Empty when the spell has no executable effects.
pub fn extract_spell_effects(def: &SpellDefinition) -> Vec<(String, Expr)> {
    def.effects
        .iter()
        .filter_map(|effect| {
            effect
                .expr
                .clone()
                .map(|expr| (effect.label().to_string(), expr))
        })
        .collect()
}

#[component]
pub fn SpellEffectsView(effects: Vec<(String, Expr)>) -> impl IntoView {
    (!effects.is_empty()).then(|| {
        view! {
            <div class="spell-effects">
                {effects
                    .into_iter()
                    .map(|(name, expr)| {
                        view! {
                            <div class="spell-effect">
                                <strong>{name}</strong>
                                <ExprView expr />
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        }
    })
}

/// Stack entry for [`AssignmentSummarizer`]: display text, optional numeric
/// value for constant folding, and compound assignment tracking (`X += rhs`).
struct SumEntry {
    text: String,
    num: Option<i32>,
    /// Raw attribute key for compound detection (e.g. "INIT.BONUS").
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

struct AssignmentSummarizer {
    stack: Vec<SumEntry>,
    iter_stack: CursorStack<Attribute>,
    i18n: I18n,
    registry: RulesRegistry,
    abilities: Vec<String>,
    skills: Vec<String>,
    saves: Vec<String>,
    equipment: Vec<String>,
    languages: Vec<&'static str>,
    other: Vec<String>,
}

impl AssignmentSummarizer {
    fn new(i18n: I18n, registry: RulesRegistry) -> Self {
        Self {
            stack: Vec::new(),
            iter_stack: CursorStack::new(),
            i18n,
            registry,
            abilities: Vec::new(),
            skills: Vec::new(),
            saves: Vec::new(),
            equipment: Vec::new(),
            languages: Vec::new(),
            other: Vec::new(),
        }
    }

    fn feature_label(&self, name: &str) -> String {
        self.registry
            .features()
            .lookup_untracked(name, |loc| loc.label().to_string())
            .unwrap_or_else(|| name.to_string())
    }

    fn pop(&mut self) -> SumEntry {
        self.stack.pop().unwrap_or(SumEntry::constant(0))
    }

    fn assign_to_attr(&mut self, attr: Attribute, value: SumEntry) {
        let attr_str = attr.to_string();
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
            Attribute::Language(name) => {
                self.languages.push(name);
            }
            _ => {
                let label = attr.display_name(self.i18n);
                // For attrs whose value is an enum index (CasterAbility,
                // CasterCoef, SlotPool), render the named label via
                // `format_value` instead of the raw integer. Compound forms
                // (`X += rhs`) keep `display` as-is since the rhs is just a
                // number and `format_value` would mis-interpret it.
                let display = if prefix.is_empty()
                    && let Some(num) = value.num
                {
                    attr.format_value(num, self.i18n)
                } else {
                    display
                };
                self.other.push(format!("{label} {prefix}{display}"));
            }
        }
    }

    fn binary_op(&mut self, bin_op: BinOp) {
        let b = self.pop();
        let a = self.pop();
        let num = a.num.zip(b.num).and_then(|(a, b)| bin_op.eval(a, b).ok());
        let op_str = bin_op.symbol();
        let text = num.map_or_else(
            || format!("{} {op_str} {}", a.text, b.text),
            |value| value.to_string(),
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

impl Interpreter<Attribute, i32, AttributeGroup> for AssignmentSummarizer {
    type Output = String;

    fn exec(&mut self, op: Op) -> Result<Option<BlockIndex>, expr::Error> {
        match op {
            Op::PushConst(n) => self.stack.push(SumEntry::constant(n)),
            Op::PushVar(var) => {
                let raw = var.to_string();
                let text = match var {
                    Attribute::Feature(name) => self.feature_label(name),
                    _ => var.display_name(self.i18n),
                };
                self.stack.push(SumEntry::var(text, raw));
            }
            Op::BinOp(BinOp::And) => {
                let b = self.pop();
                let a = self.pop();
                let op = self.i18n.tr("expr-and");
                self.stack.push(SumEntry {
                    text: format!("{} {op} {}", a.text, b.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::BinOp(BinOp::Or) => {
                let b = self.pop();
                let a = self.pop();
                let op = self.i18n.tr("expr-or");
                self.stack.push(SumEntry {
                    text: format!("{} {op} {}", a.text, b.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::BinOp(bin_op) => self.binary_op(bin_op),
            Op::AvgHp => {
                let a = self.pop();
                let num = a.num.map(expr::avg_hp);
                let text = num.map_or_else(|| format!("avg_hp({})", a.text), |n| n.to_string());
                self.stack.push(SumEntry {
                    text,
                    num,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::AssignVar(attr) => {
                let value = self.pop();
                self.assign_to_attr(attr, value);
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
            Op::Not => {
                let a = self.pop();
                let op = self.i18n.tr("expr-not");
                self.stack.push(SumEntry {
                    text: format!("{op} {}", a.text),
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
            // if(): evaluate both branches to collect all possible assignments.
            // Exception: when condition is a known constant(0) — this is a loop
            // termination from Next. Don't re-enter the body.
            Op::EvalIf(then_idx, else_idx) => {
                let cond = self.pop();
                if cond.num != Some(0) && then_idx != BLOCK_NOOP && then_idx != BLOCK_ERROR {
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
            // Dice/roll ops: format as dice notation
            Op::Roll => {
                let sides = self.pop();
                let amount = self.pop();
                let text = format!("{}d{}", amount.text, sides.text);
                self.stack.push(SumEntry {
                    text,
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::Sum => {
                let sides = self.pop();
                let amount = self.pop();
                let text = format!("{}d{}", amount.text, sides.text);
                self.stack.push(SumEntry {
                    text,
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::Explode => {
                let sides = self.pop();
                let amount = self.pop();
                let text = format!("{}d{}!", amount.text, sides.text);
                self.stack.push(SumEntry {
                    text,
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::KeepMax(n) => {
                let top = self.pop();
                self.stack.push(SumEntry {
                    text: format!("{}kh{n}", top.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::KeepMin(n) => {
                let top = self.pop();
                self.stack.push(SumEntry {
                    text: format!("{}kl{n}", top.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::DropMax(n) => {
                let top = self.pop();
                self.stack.push(SumEntry {
                    text: format!("{}dh{n}", top.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::DropMin(n) => {
                let top = self.pop();
                self.stack.push(SumEntry {
                    text: format!("{}dl{n}", top.text),
                    num: None,
                    raw_key: None,
                    compound: None,
                });
            }
            Op::Each(subgrp) => {
                let cursor = GroupCursor::build(&StaticAttrSource, &subgrp);
                let live = cursor.is_live();
                if live {
                    self.iter_stack.push(cursor);
                }
                self.stack.push(SumEntry::constant(live as i32));
            }
            Op::Next => {
                let more = self
                    .iter_stack
                    .top_mut()
                    .map(|cursor| cursor.advance())
                    .unwrap_or(false);
                if !more {
                    let _ = self.iter_stack.pop();
                }
                self.stack.push(SumEntry::constant(more as i32));
            }
            Op::PushGroup(_grp, col) => {
                if let Some(var) = self
                    .iter_stack
                    .top()
                    .ok()
                    .and_then(|cursor| cursor.col(col).copied())
                {
                    let raw = var.to_string();
                    let text = var.display_name(self.i18n);
                    self.stack.push(SumEntry::var(text, raw));
                } else {
                    self.stack.push(SumEntry::constant(0));
                }
            }
            Op::AssignGroup(_grp, col) => {
                if let Some(attr) = self
                    .iter_stack
                    .top()
                    .ok()
                    .and_then(|cursor| cursor.col(col).copied())
                {
                    let value = self.pop();
                    self.assign_to_attr(attr, value);
                }
            }
            Op::Tier(count) => {
                self.pop(); // var
                for _ in 0..count {
                    self.pop();
                    self.pop();
                }
                self.stack.push(SumEntry::constant(0));
            }
        }
        Ok(None)
    }

    fn finish(self) -> Result<Self::Output, expr::Error> {
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
        if !self.languages.is_empty() {
            parts.push(self.languages.join(", "));
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
    i18n: I18n,
    registry: RulesRegistry,
) -> String {
    assignments
        .iter()
        .filter_map(|a| a.expr.run(AssignmentSummarizer::new(i18n, registry)).ok())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Stable per-feature shape used by `<ReferenceFeaturesView>`. Locale
/// text and Expr-derived strings (label, description, prerequisites,
/// assignments) are resolved from the registry by the per-row view —
/// keys remain valid across locale switches, so `<For>` keeps DOM
/// nodes alive and only the text content updates.
#[derive(Clone)]
pub struct FeatureKey {
    pub name: String,
    pub category: FeatureCategory,
    /// `true` when the feature definition has `prerequisites: Some(_)`.
    /// The actual prerequisite text is resolved per-row via i18n.
    pub has_prerequisites: bool,
    /// `true` when the feature has `assign: Some(_)`.
    pub has_assignments: bool,
    pub spells: FeatureSpells,
    pub choices: Option<Vec<ChoiceFieldKey>>,
    pub package: String,
}

#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "ref.collect_feature_views", skip_all)
)]
pub fn collect_feature_views<'a>(
    features: impl Iterator<Item = &'a FeatureDefinition>,
) -> Vec<FeatureKey> {
    features
        .map(|feat| FeatureKey {
            name: feat.name.to_string(),
            category: feat.category,
            has_prerequisites: feat.prerequisites.is_some(),
            has_assignments: feat.assign.as_deref().is_some_and(|a| !a.is_empty()),
            spells: FeatureSpells::from_feature(feat),
            choices: feature_choices(&feat.name, &feat.actions),
            package: feat.package.to_string(),
        })
        .collect()
}

/// Render a list of reference features.
#[component]
pub fn ReferenceFeaturesView(
    features: Vec<FeatureKey>,
    #[prop(optional)] anchors: bool,
) -> impl IntoView {
    if features.is_empty() {
        return None;
    }
    Some(view! {
        <div class="reference-features">
            <For
                each=move || features.clone()
                key=|key| key.name.clone()
                children=move |key| view! { <FeatureRowView key anchors /> }
            />
        </div>
    })
}

/// Per-feature row. All locale-dep text (label, description, category,
/// prerequisites, assignments) is derived from the registry inside this
/// child scope, so the row's DOM stays mounted across locale switches
/// and only text/HTML content updates.
#[component]
fn FeatureRowView(key: FeatureKey, anchors: bool) -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let id = anchors.then(|| format!("feat-{}", key.name));
    let category = key.category;
    let has_prerequisites = key.has_prerequisites;
    let has_assignments = key.has_assignments;
    let name = key.name.clone();
    let package = key.package.clone();

    let (label, description) = registry.features().label_desc(&name, &name);
    let category_label = Signal::derive(move || i18n.tr(category.tr_key()));

    let prerequisites = Signal::derive({
        let name = name.clone();
        move || {
            registry
                .features()
                .lookup(&name, |loc| {
                    loc.data
                        .prerequisites
                        .as_ref()
                        .and_then(|p| p.run(AssignmentSummarizer::new(i18n, registry)).ok())
                        .unwrap_or_default()
                })
                .unwrap_or_default()
        }
    });
    let assignments = Signal::derive({
        let name = name.clone();
        move || {
            registry
                .features()
                .lookup(&name, |loc| {
                    loc.data
                        .assign
                        .as_deref()
                        .map(|a| summarize_assignments(a, i18n, registry))
                        .unwrap_or_default()
                })
                .unwrap_or_default()
        }
    });

    view! {
        <div class="reference-feature" id=id>
            <div class="reference-row-head">
                <h3>{move || label.get()}</h3>
                {(!package.is_empty())
                    .then(|| {
                        let label = move || registry.package_display_name(&package);
                        view! { <span class="entry-badge package-badge">{label}</span> }
                    })}
            </div>
            <p class="feature-prerequisites">
                {move || category_label.get()}
                {has_prerequisites
                    .then(|| {
                        view! {
                            {" · "}
                            {move_tr!("ref-prerequisites")}
                            {": "}
                            {move || prerequisites.get()}
                        }
                    })}
            </p>
            <Markdown text=description />
            {has_assignments
                .then(|| view! { <p class="feature-assignments">{move || assignments.get()}</p> })}
            <FeatureSpellsView spells=key.spells />
            <FeatureChoicesView choices=key.choices />
        </div>
    }
}

#[component]
pub fn FeatureChoicesView(choices: Option<Vec<ChoiceFieldKey>>) -> impl IntoView {
    choices.map(|fields| {
        view! {
            <div class="feature-choices-inline">
                <For
                    each=move || fields.clone()
                    key=|key| key.field_name.clone()
                    children=move |key| view! { <ChoiceFieldRow key /> }
                />
            </div>
        }
    })
}

#[component]
fn ChoiceFieldRow(key: ChoiceFieldKey) -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let field_key = LocaleKey::flat_field(&key.feat_name, &key.field_name)
        .as_str()
        .to_string();
    let (field_label, field_description) = registry
        .features()
        .label_desc(field_key, key.field_name.clone());
    let field_description_for_md = field_description.clone();
    let options = key.options;
    view! {
        <div class="feature-choice-field">
            <strong>{move || field_label.get()}</strong>
            {move || {
                let desc = field_description.get();
                (!desc.is_empty())
                    .then(|| view! { <Markdown text=field_description_for_md.clone() /> })
            }}
            <div class="feature-choice-options">
                <For
                    each=move || options.clone()
                    key=|key| key.name.clone()
                    children=move |key| view! { <ChoiceOptionRow key /> }
                />
            </div>
        </div>
    }
}

#[component]
fn ChoiceOptionRow(key: ChoiceOptionKey) -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let opt_key = LocaleKey::flat_field_option(&key.feat_name, &key.field_name, &key.name)
        .as_str()
        .to_string();
    let (opt_label, opt_description) = registry.features().label_desc(opt_key, key.name.clone());
    let opt_description_for_md = opt_description.clone();

    let feat_name = key.feat_name.clone();
    let field_name = key.field_name.clone();
    let name = key.name.clone();

    // Pull `level`, `cost`, `effects`, and the field's `cost_unit` from the
    // feature definition on render — keeps the view-key small and lets
    // structural changes (rare) flow through naturally.
    view! {
        <div class="feature-choice-entry">
            <strong>{move || opt_label.get()}</strong>
            {move || {
                registry
                    .features()
                    .lookup(
                        &feat_name,
                        |loc| {
                            let action_def = loc.data.actions.get(field_name.as_str())?;
                            let ChoiceOptions::List(list) = &action_def.options else {
                                return None;
                            };
                            let opt = list.iter().find(|o| *o.name == name)?;
                            let level = opt.level;
                            let cost = opt.cost;
                            let unit = action_def.cost.clone();
                            let effects: Vec<(String, Expr)> = opt
                                .effects
                                .iter()
                                .filter_map(|effect| {
                                    effect
                                        .expr
                                        .clone()
                                        .map(|expr| (effect.label().to_string(), expr))
                                })
                                .collect();
                            Some(
                                view! {
                                    {(level > 0 || (cost > 0 && unit.is_some()))
                                        .then(|| {
                                            view! {
                                                {" ("}
                                                {(level > 0)
                                                    .then(|| {
                                                        view! {
                                                            {move_tr!(
                                                                "ref-spell-min-level", { "level" => level.to_string() }
                                                            )}
                                                        }
                                                    })}
                                                {(cost > 0)
                                                    .then(|| {
                                                        let u = unit.clone().unwrap_or_default();
                                                        let sep = if level > 0 { ", " } else { "" };
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
                                    {(!effects.is_empty())
                                        .then(|| {
                                            view! {
                                                <div class="spell-effects">
                                                    {effects
                                                        .into_iter()
                                                        .map(|(name, expr)| {
                                                            view! {
                                                                <div class="spell-effect">
                                                                    <strong>{name}</strong>
                                                                    <ExprView expr />
                                                                </div>
                                                            }
                                                        })
                                                        .collect_view()}
                                                </div>
                                            }
                                        })}
                                },
                            )
                        },
                    )
            }}
            {move || {
                let desc = opt_description.get();
                (!desc.is_empty())
                    .then(|| view! { <Markdown text=opt_description_for_md.clone() /> })
            }}
        </div>
    }
}

#[component]
pub fn FeatureSpellsView(spells: FeatureSpells) -> impl IntoView {
    match spells {
        FeatureSpells::Link(list_name) => EitherOf3::A(view! {
            <p class="feature-spell-link">
                <Ref href=format!("/r/spell/{list_name}")>{move_tr!("ref-spell-list-link")}</Ref>
            </p>
        }),
        FeatureSpells::Inline(rows) => EitherOf3::B(view! {
            <div class="feature-spells-inline">
                <For
                    each=move || rows.clone()
                    key=|key| key.name.clone()
                    children=move |key| view! { <InlineSpellRowView key /> }
                />
            </div>
        }),
        FeatureSpells::None => EitherOf3::C(()),
    }
}

#[component]
fn InlineSpellRowView(key: InlineSpellKey) -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let name = key.name.clone();
    let entry = key.entry;
    let min_level = entry.min_level;
    let sticky = entry.sticky;

    let (label, description) = registry.spells().label_desc(&name, &name);

    view! {
        <div class="feature-spell-entry">
            <strong>{move || label.get()}</strong>
            {move || {
                let extracted = registry
                    .spells()
                    .lookup(
                        &name,
                        |loc| {
                            (
                                loc.data.level,
                                loc.data.meta(),
                                loc.data.components.clone(),
                                extract_spell_effects(loc.data),
                            )
                        },
                    );
                extracted
                    .map(|(level, meta, components, effects)| {
                        let level_text = if level == 0 {
                            move_tr!("ref-cantrips-level")
                        } else {
                            move_tr!("ref-spell-level", {"level" => level})
                        };
                        view! {
                            {" ("}
                            {level_text}
                            {sticky
                                .then(|| {
                                    view! {
                                        {", "}
                                        {move_tr!("ref-spell-always-ready")}
                                    }
                                })}
                            {(min_level > 0)
                                .then(|| {
                                    view! {
                                        {", "}
                                        {move_tr!("ref-spell-min-level", {"level" => min_level})}
                                    }
                                })}
                            {")"}
                            <SpellInfoBar meta=meta components=components />
                            <SpellEffectsView effects />
                        }
                    })
            }}
            <Markdown text=description />
        </div>
    }
}
