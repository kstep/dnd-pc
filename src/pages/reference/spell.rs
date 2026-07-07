use std::{
    collections::{BTreeMap, HashSet},
    fmt,
    str::FromStr,
};

use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{hooks::use_params, params::Params};
use strum::{IntoEnumIterator, VariantArray};
use wasm_bindgen::JsCast;

use crate::{
    components::{markdown::Markdown, spell_info_bar::SpellInfoBar, spinner::Spinner},
    hooks::{use_hash_href, use_query_signal},
    model::Translatable,
    pages::reference::{
        RefSidebarEntries, ReferenceSidebar, SpellEffectsView, extract_spell_effects,
    },
    rules::{IndexEntry, RulesRegistry, SpellCategory, SpellsList},
};

const CATEGORY_COUNT: usize = SpellCategory::VARIANTS.len();
type CategoryCounts = [u32; CATEGORY_COUNT];
type SpellEntries = Vec<(u32, String, SpellCategory)>;

/// URL-serialisable wrapper around the chip-filter set.
/// Renders as comma-separated variant names (`?cat=Damage,Control`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct CategoryFilter(HashSet<SpellCategory>);

impl FromStr for CategoryFilter {
    type Err = strum::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut set = HashSet::new();
        for part in s.split(',').filter(|p| !p.is_empty()) {
            set.insert(part.parse()?);
        }
        Ok(Self(set))
    }
}

impl fmt::Display for CategoryFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Stable order so URLs are deterministic.
        let mut sorted: Vec<_> = self.0.iter().copied().collect();
        sorted.sort_by_key(|c| *c as usize);
        for (idx, cat) in sorted.iter().enumerate() {
            if idx > 0 {
                f.write_str(",")?;
            }
            cat.fmt(f)?;
        }
        Ok(())
    }
}

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
            .entry_label_desc(IndexEntry::Spell(&list_name()))
            .0
            .get()
    });

    // Filter state lives in the URL (`?cat=Damage,Control`) so the view is
    // shareable.
    let (filter, set_filter) = use_query_signal::<CategoryFilter>("cat");
    let selected = Signal::derive(move || filter.get().map(|f| f.0).unwrap_or_default());

    // Counts depend only on the spell list (path), not on the filter; recomputing
    // on every chip toggle would clone Strings for nothing.
    let entries_counts: Memo<Option<(SpellEntries, CategoryCounts)>> = Memo::new(move |_| {
        let name = list_name();
        if name.is_empty() {
            return None;
        }
        let path = SpellsList::ref_path(&name);
        registry.with_spell_list(&path, |iter| {
            let entries: SpellEntries = iter
                .map(|spell| (spell.level, spell.name.to_string(), spell.category))
                .collect();
            let counts =
                entries
                    .iter()
                    .fold([0u32; CATEGORY_COUNT], |mut acc, (_, _, category)| {
                        acc[*category as usize] += 1;
                        acc
                    });
            (entries, counts)
        })
    });

    let detail = move || {
        if list_name().is_empty() {
            return view! {
                <div class="reference-empty">
                    <p>{move_tr!("ref-select-spell-list")}</p>
                </div>
            }
            .into_any();
        }

        let Some((entries, counts)) = entries_counts.get() else {
            return ().into_any();
        };

        let active = selected.get();
        let mut by_level_map: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for (level, spell_name, category) in entries {
            if !active.is_empty() && !active.contains(&category) {
                continue;
            }
            by_level_map.entry(level).or_default().push(spell_name);
        }
        let by_level: Vec<(u32, Vec<String>)> = by_level_map.into_iter().collect();
        let levels: Vec<u32> = by_level.iter().map(|(level, _)| *level).collect();

        view! {
            <Title text=current_label />
            <div class="reference-detail">
                <h1>{move || current_label.get()}</h1>
                <div class="spell-cat-filter">
                    <For
                        each=move || SpellCategory::iter().filter(move |c| counts[*c as usize] > 0)
                        key=|category| *category
                        children=move |category| {
                            view! { <SpellCatChip category counts selected set_filter /> }
                        }
                    />
                </div>
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
                            <section class="spell-level-section">
                                <h2 id=section_id>{heading}</h2>
                                <div class="reference-features">
                                    <For
                                        each=move || spells.clone()
                                        key=|spell_name| spell_name.clone()
                                        children=move |spell_name| {
                                            view! { <SpellRowView name=spell_name /> }
                                        }
                                    />
                                </div>
                            </section>
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
                        names=Signal::derive(move || registry.spell_list_names())
                        kind=|n| IndexEntry::Spell(n)
                    />
                </ReferenceSidebar>
                <main class="reference-main">{detail}</main>
            </div>
        </div>
    }
}

#[component]
fn SpellCatChip(
    category: SpellCategory,
    counts: CategoryCounts,
    selected: Signal<HashSet<SpellCategory>>,
    set_filter: SignalSetter<Option<CategoryFilter>>,
) -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();
    let tr_key = category.tr_key();
    let label = Signal::derive(move || i18n.tr(tr_key));
    let count = counts[category as usize];
    let active = move || selected.get().contains(&category);
    view! {
        <button
            class="entry-badge spell-cat-chip"
            class:is-active=active
            on:click=move |_| {
                let mut set = selected.get_untracked();
                if !set.insert(category) {
                    set.remove(&category);
                }
                set_filter.set((!set.is_empty()).then_some(CategoryFilter(set)));
            }
        >
            {label}
            <span class="spell-cat-count">{count}</span>
        </button>
    }
}

#[component]
fn SpellRowView(name: String) -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let anchor_id = format!("spell-{name}");
    let (label, description) = registry.spells().label_desc(&name, &name);
    // Package is structural (locale-independent) — read separately from the
    // locale-overlaid lookup below so it can sit next to the heading.
    let package = Signal::derive({
        let name = name.clone();
        move || {
            registry
                .with_spells_index(|index| {
                    index.get(name.as_str()).map(|def| def.package.to_string())
                })
                .unwrap_or_default()
        }
    });

    view! {
        <div class="reference-feature" id=anchor_id>
            <div class="reference-row-head">
                <h3>{move || label.get()}</h3>
                {move || {
                    let package = package.get();
                    (!package.is_empty())
                        .then(|| {
                            let label = registry.package_display_name(&package);
                            view! { <span class="entry-badge package-badge">{label}</span> }
                        })
                }}
            </div>
            {move || {
                registry
                    .spells()
                    .lookup(
                        &name,
                        |loc| {
                            let meta = loc.data.meta();
                            let components = loc.data.components.clone();
                            let effects = extract_spell_effects(loc.data);
                            view! {
                                <SpellInfoBar meta=meta components=components />
                                <SpellEffectsView effects />
                            }
                        },
                    )
            }}
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
