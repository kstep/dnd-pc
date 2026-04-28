use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{hooks::use_params, params::Params};
use wasm_bindgen::JsCast;

use crate::{
    components::{markdown::Markdown, spell_info_bar::SpellInfoBar, spinner::Spinner},
    hooks::use_hash_href,
    pages::reference::{
        RefSidebarEntries, ReferenceSidebar, SpellEffectsView, extract_spell_effects,
    },
    rules::{IndexEntry, RulesRegistry, SpellsList},
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct SpellRefParams {
    list: Option<String>,
}

#[component]
pub fn SpellReference() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let params = use_params::<SpellRefParams>();

    let list_name = move || params.get().ok().and_then(|p| p.list).unwrap_or_default();

    Effect::new(move || {
        let name = list_name();
        if !name.is_empty() {
            let path = SpellsList::ref_path(&name);
            registry.fetch_spell_list(&path);
        }
    });

    let current_label = Signal::derive(move || {
        registry
            .index()
            .entry_label_desc(IndexEntry::Spell(&list_name()))
            .0
            .get()
    });

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

        let path = SpellsList::ref_path(&name);

        // Pull only stable identity (level + name) out of the registry —
        // labels, descriptions, meta, and effects are resolved per-row inside
        // <For> children so locale switches patch text in place.
        let by_level: Vec<(u32, Vec<String>)> = registry
            .with_spell_list(&path, |iter| {
                let mut map: BTreeMap<u32, Vec<String>> = BTreeMap::new();
                for spell in iter {
                    map.entry(spell.level)
                        .or_default()
                        .push(spell.name.to_string());
                }
                map.into_iter().collect()
            })
            .unwrap_or_default();

        if by_level.is_empty() {
            return ().into_any();
        }

        let title = registry
            .index()
            .entry_label_desc(IndexEntry::Spell(&name))
            .0;
        let levels: Vec<u32> = by_level.iter().map(|(level, _)| *level).collect();
        view! {
            <Title text=title />
            <div class="reference-detail">
                <h1>{move || title.get()}</h1>
                <For
                    each=move || by_level.clone()
                    key=|(level, _)| *level
                    children=move |(level, spells)| {
                        let section_id = format!("spell-level-{level}");
                        let heading = if level == 0 {
                            move_tr!("ref-cantrips")
                        } else {
                            move_tr!("ref-spell-level", {"level" => level})
                        };
                        view! {
                            <h2 id=section_id>{heading}</h2>
                            <div class="reference-features">
                                <For
                                    each=move || spells.clone()
                                    key=|spell_name| spell_name.clone()
                                    children=move |spell_name| view! {
                                        <SpellRowView name=spell_name />
                                    }
                                />
                            </div>
                        }
                    }
                />
            </div>
            <SpellLevelNav levels />
        }
        .into_any()
    };

    let loading = Signal::derive(move || {
        let name = list_name();
        !name.is_empty()
            && registry
                .with_spell_list(&SpellsList::ref_path(&name), |_| ())
                .is_none()
    });

    view! {
        <Spinner loading />
        <Title text=move_tr!(i18n, "ref-spells") />
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    <RefSidebarEntries
                        names=Signal::derive(move || registry.with_spell_entries(|entries| {
                            entries.values().map(|entry| entry.name.to_string()).collect()
                        }))
                        kind=|n| IndexEntry::Spell(n)
                    />
                </ReferenceSidebar>
                <main class="reference-main">
                    {detail}
                </main>
            </div>
        </div>
    }
}

#[component]
fn SpellRowView(name: String) -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let anchor_id = format!("spell-{name}");
    let (label, description) = registry.spells().label_desc(&name, &name);

    view! {
        <div class="reference-feature" id=anchor_id>
            <h3>{move || label.get()}</h3>
            {move || registry.spells().lookup(&name, |loc| {
                let meta = loc.data.meta();
                let effects = extract_spell_effects(loc.data);
                view! {
                    <SpellInfoBar meta=meta />
                    <SpellEffectsView effects />
                }
            })}
            <Markdown text=description />
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

    let details_ref = NodeRef::<leptos::html::Details>::new();
    let close = move |event: web_sys::MouseEvent| {
        let target = event
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok());
        if target.is_some_and(|t| t.closest("a").ok().flatten().is_some())
            && let Some(details) = details_ref.get()
        {
            details.set_open(false);
        }
    };

    view! {
        <details class="floating-nav" node_ref=details_ref on:click=close>
            <summary>"#"</summary>
            {items}
        </details>
    }
}
