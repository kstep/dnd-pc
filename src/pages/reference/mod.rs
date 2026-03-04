pub mod background;
pub mod class;
pub mod race;
pub mod sidebar;
pub mod spell;
use std::collections::BTreeMap;

use leptos::{either::EitherOf3, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::components::A;
pub use sidebar::ReferenceSidebar;

use crate::{
    BASE_URL,
    rules::{ChoiceOptions, FieldDefinition, FieldKind, SpellList},
};

pub struct InlineSpell {
    pub label: String,
    pub level: u32,
    pub min_level: u32,
    pub sticky: bool,
    pub description: String,
}

pub enum FeatureSpells {
    None,
    Link(String),
    Inline(Vec<InlineSpell>),
}

impl FeatureSpells {
    pub fn from_spell_list(list: Option<&SpellList>) -> Self {
        match list {
            Some(SpellList::Ref { from }) => {
                let list_name = from
                    .strip_prefix("spells/")
                    .and_then(|s| s.strip_suffix(".json"))
                    .unwrap_or(from);
                Self::Link(list_name.to_string())
            }
            Some(SpellList::Inline(spells)) if !spells.is_empty() => Self::Inline(
                spells
                    .iter()
                    .map(|s| InlineSpell {
                        label: s.label().to_string(),
                        level: s.level,
                        min_level: s.min_level,
                        sticky: s.sticky,
                        description: s.description.clone(),
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
}

pub struct ChoiceFieldView {
    pub label: String,
    pub description: String,
    pub cost_unit: Option<String>,
    pub options: Vec<InlineChoiceOption>,
}

pub fn feature_choices(fields: &BTreeMap<String, FieldDefinition>) -> Option<Vec<ChoiceFieldView>> {
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
                        move_tr!("ref-spell-level", {"level" => spell.level.to_string()})
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
                                {", "}{move_tr!("ref-spell-min-level", {"level" => min_level.to_string()})}
                            })}
                            {")"}
                            {(!spell.description.is_empty()).then(|| view! {
                                <p>{spell.description}</p>
                            })}
                        </div>
                    }
                }).collect_view()}
            </div>
        }),
        FeatureSpells::None => EitherOf3::C(()),
    }
}
