use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, params::Params};

use super::ReferenceSidebar;
use crate::{BASE_URL, rules::RulesRegistry};

#[derive(Params, Clone, Debug, PartialEq)]
struct SpellRefParams {
    list: Option<String>,
}

#[component]
pub fn SpellReference() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let params = use_params::<SpellRefParams>();

    let list_name = move || params.get().ok().and_then(|p| p.list).unwrap_or_default();

    Effect::new(move || {
        let name = list_name();
        if !name.is_empty() {
            let path = format!("spells/{name}.json");
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

        let path = format!("spells/{name}.json");

        registry
            .with_spell_list_tracked(&path, |spells| {
                let title = registry.with_spell_entries(|entries| {
                    entries
                        .iter()
                        .find(|e| e.name == name)
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
                }
                struct SpellGroup {
                    level: u32,
                    spells: Vec<SpellEntry>,
                }
                let mut by_level: Vec<SpellGroup> = Vec::new();
                for spell in spells {
                    let level = spell.level;
                    let entry = SpellEntry {
                        name: spell.name.clone(),
                        label: spell.label().to_string(),
                        description: spell.description.clone(),
                        min_level: spell.min_level,
                        sticky: spell.sticky,
                    };
                    if let Some(group) = by_level.iter_mut().find(|g| g.level == level) {
                        group.spells.push(entry);
                    } else {
                        by_level.push(SpellGroup {
                            level,
                            spells: vec![entry],
                        });
                    }
                }
                by_level.sort_by_key(|g| g.level);

                let title_for_heading = title.clone();
                view! {
                    <Title text=title />
                    <div class="reference-detail">
                        <h1>{title_for_heading}</h1>

                        {by_level.into_iter().map(|group| {
                            let level = group.level;
                            let spells = group.spells;
                            let heading = if level == 0 {
                                move_tr!("ref-cantrips-level")
                            } else {
                                move_tr!("ref-spell-level", {"level" => level.to_string()})
                            };
                            view! {
                                <h2>{heading}</h2>
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
                                                            parts.push(move_tr!("ref-spell-min-level", {"level" => min_level.to_string()}));
                                                        }
                                                        view! {
                                                            <span class="spell-prereq">
                                                                {" ("}{move || parts.iter().map(|p| p.get()).collect::<Vec<_>>().join(", ")}{")"}
                                                            </span>
                                                        }
                                                    })}
                                                </h3>
                                                <p>{spell.description}</p>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }
                .into_any()
            })
            .unwrap_or_else(|| {
                view! { <p class="reference-loading">{move_tr!("ref-loading")}</p> }.into_any()
            })
    };

    view! {
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    {move || registry.with_spell_entries(|entries| {
                        entries.iter().map(|entry| {
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
