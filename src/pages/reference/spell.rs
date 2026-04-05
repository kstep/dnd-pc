use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, params::Params};

use super::ReferenceSidebar;
use crate::{
    BASE_URL,
    components::{expr_view::ExprView, spinner::Spinner},
    expr::Expr,
    hooks::use_hash_href,
    model::Attribute,
    rules::{RulesRegistry, SpellList},
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct SpellRefParams {
    list: Option<String>,
}

#[component]
pub fn SpellReference() -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();
    let registry = expect_context::<RulesRegistry>();
    let params = use_params::<SpellRefParams>();

    let list_name = move || params.get().ok().and_then(|p| p.list).unwrap_or_default();

    Effect::new(move || {
        let name = list_name();
        if !name.is_empty() {
            let path = SpellList::ref_path(&name);
            registry.fetch_spell_list_tracked(&path);
        }
    });

    let current_label = Signal::derive(move || registry.spell_label_by_name(&list_name()));

    let detail = move || {
        let name = list_name();

        if name.is_empty() {
            return view! {
                <div class="reference-empty">
                    <p>{move_tr!("ref-select-spell-list")}</p>
                </div>
            }
            .into_any();
        }

        let path = SpellList::ref_path(&name);

        registry
            .with_spell_list_tracked(&path, |spells| {
                let title = registry.with_spell_entries(|entries| {
                    entries
                        .get(name.as_str())
                        .map(|e| e.label().to_string())
                        .unwrap_or_else(|| name.clone())
                });

                // Group spells by level
                struct SpellEntry {
                    name: String,
                    label: String,
                    description: String,
                    min_level: u32,
                    sticky: bool,
                    effects: Vec<(String, Expr<Attribute>)>,
                }
                struct SpellGroup {
                    level: u32,
                    spells: Vec<SpellEntry>,
                }
                let mut by_level_map: BTreeMap<u32, Vec<SpellEntry>> =
                    BTreeMap::new();
                for spell in spells.values() {
                    by_level_map
                        .entry(spell.level)
                        .or_default()
                        .push(SpellEntry {
                            name: spell.name.clone(),
                            label: spell.label().to_string(),
                            description: spell.description.clone(),
                            min_level: spell.min_level,
                            sticky: spell.sticky,
                            effects: spell
                                .effects
                                .iter()
                                .map(|e| (e.label().to_string(), e.expr.clone()))
                                .collect(),
                        });
                }
                let by_level: Vec<SpellGroup> = by_level_map
                    .into_iter()
                    .map(|(level, spells)| SpellGroup { level, spells })
                    .collect();

                let levels: Vec<u32> =
                    by_level.iter().map(|group| group.level).collect();
                let title_for_heading = title.clone();
                view! {
                    <Title text=title />
                    <div class="reference-detail">
                        <h1>{title_for_heading}</h1>

                        {by_level.into_iter().map(|group| {
                            let level = group.level;
                            let spells = group.spells;
                            let section_id = format!("spell-level-{level}");
                            let heading = if level == 0 {
                                move_tr!("ref-cantrips")
                            } else {
                                move_tr!("ref-spell-level", {"level" => level})
                            };
                            view! {
                                <h2 id=section_id>{heading}</h2>
                                <div class="reference-features">
                                    {spells.into_iter().map(|spell| {
                                        let anchor_id = format!("spell-{}", spell.name);
                                        let min_level = spell.min_level;
                                        let sticky = spell.sticky;
                                        view! {
                                            <div class="reference-feature" id=anchor_id>
                                                <h3>
                                                    {spell.label}
                                                    {(min_level > 0 || sticky).then(|| {
                                                        let mut parts = Vec::new();
                                                        if sticky {
                                                            parts.push(move_tr!("ref-spell-always-ready"));
                                                        }
                                                        if min_level > 0 {
                                                            parts.push(move_tr!("ref-spell-min-level", {"level" => min_level}));
                                                        }
                                                        view! {
                                                            <span class="spell-prereq">
                                                                {" ("}{move || parts.iter().map(|p| p.get()).collect::<Vec<_>>().join(", ")}{")"}
                                                            </span>
                                                        }
                                                    })}
                                                </h3>
                                                <p>{spell.description}</p>
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
                            }
                        }).collect_view()}
                    </div>
                    <SpellLevelNav levels />
                }
                .into_any()
            })
            .unwrap_or_else(|| ().into_any())
    };

    let loading = Signal::derive(move || {
        let name = list_name();
        !name.is_empty()
            && registry
                .with_spell_list_tracked(&SpellList::ref_path(&name), |_| ())
                .is_none()
    });

    view! {
        <Spinner loading />
        <Title text=Signal::derive(move || i18n.tr("ref-spells")) />
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    {move || registry.with_spell_entries(|entries| {
                        entries.values().map(|entry| {
                            let name = entry.name.clone();
                            let label = entry.label().to_string();
                            view! {
                                <A href=format!("{BASE_URL}/r/spell/{name}") attr:class="reference-nav-item">
                                    {label}
                                </A>
                            }
                        }).collect_view()
                    })}
                </ReferenceSidebar>
                <main class="reference-main">
                    {detail}
                </main>
            </div>
        </div>
    }
}

#[component]
fn SpellLevelNav(levels: Vec<u32>) -> impl IntoView {
    let hash_href = use_hash_href();

    let items = levels
        .into_iter()
        .map(|level| {
            let href = hash_href(&format!("spell-level-{level}"));
            let label = if level == 0 {
                move_tr!("ref-cantrips")
            } else {
                move_tr!("ref-spell-level", {"level" => level})
            };
            view! {
                <a class="floating-nav-btn" href=href title=label rel="external">
                    {level}
                </a>
            }
        })
        .collect_view();

    view! {
        <details class="floating-nav">
            <summary>"#"</summary>
            {items}
        </details>
    }
}
