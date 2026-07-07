use std::time::Duration;

use leptos::{leptos_dom::helpers::debounce, prelude::*};
use leptos_fluent::{move_tr, tr};
use leptos_meta::Title;
use leptos_router::{hooks::use_params, params::Params};
use regex::RegexBuilder;

use crate::{
    components::ref_link::Ref,
    hooks::use_query_signal,
    model::{FeatureCategory, Translatable},
    pages::reference::{ReferenceFeaturesView, ReferenceSidebar, collect_feature_views},
    rules::RulesRegistry,
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct FeatureRefParams {
    category: Option<String>,
}

#[component]
pub fn FeatureReference() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let params = use_params::<FeatureRefParams>();
    let (search, set_search) = use_query_signal::<String>("q");
    let mut on_search = debounce(Duration::from_millis(300), move |value: String| {
        set_search.set(if value.is_empty() { None } else { Some(value) });
    });

    let category = move || {
        params
            .get()
            .ok()
            .and_then(|p| p.category)
            .and_then(|name| name.parse::<FeatureCategory>().ok())
    };

    let current_label = Signal::derive(move || {
        category()
            .map(|category| i18n.tr(category.tr_key()))
            .unwrap_or_else(|| tr!(i18n, "feat-cat-all"))
    });

    let features_view = move || {
        let category = category();
        let query = search.get().unwrap_or_default();
        let regex = if query.is_empty() {
            None
        } else {
            RegexBuilder::new(&query)
                .case_insensitive(true)
                .build()
                .or_else(|_| {
                    RegexBuilder::new(&regex::escape(&query))
                        .case_insensitive(true)
                        .build()
                })
                .ok()
        };

        registry.with_features_index(|idx| {
            let filtered = idx.values().filter(|feat| {
                let category_match = match category {
                    Some(category) => feat.category == category,
                    None => feat.is_selectable(),
                };
                if !category_match {
                    return false;
                }
                match &regex {
                    Some(regex) => {
                        let (label, description) = registry
                            .features()
                            .lookup_untracked(&feat.name, |loc| {
                                (loc.label().to_string(), loc.description().to_string())
                            })
                            .unwrap_or_else(|| (feat.name.to_string(), String::new()));
                        regex.is_match(&label) || regex.is_match(&description)
                    }
                    None => true,
                }
            });
            let features = collect_feature_views(filtered);
            if features.is_empty() {
                None
            } else {
                Some(view! { <ReferenceFeaturesView features /> })
            }
        })
    };

    let render_category = move |category: FeatureCategory| {
        let name = category.to_string();
        let label = Signal::derive(move || i18n.tr(category.tr_key()));
        view! {
            <Ref href=format!("/r/feature/{name}") attr:class="reference-nav-item">
                {move || label.get()}
            </Ref>
        }
    };

    view! {
        <Title text=move_tr!(i18n, "ref-features") />
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    <Ref href="/r/feature" attr:class="reference-nav-item" exact=true>
                        {move_tr!("feat-cat-all")}
                    </Ref>
                    <For
                        each=|| FeatureCategory::USER_FACING.iter().copied()
                        key=|c| *c
                        children=render_category
                    />
                </ReferenceSidebar>
                <div class="reference-feature-page">
                    <div class="reference-feature-header">
                        <input
                            type="search"
                            class="reference-search"
                            placeholder=move_tr!("ref-search-feature")
                            prop:value=move || search.get().unwrap_or_default()
                            on:input=move |event| on_search(event_target_value(&event))
                        />
                    </div>
                    <main class="reference-main">{features_view}</main>
                </div>
            </div>
        </div>
    }
}
