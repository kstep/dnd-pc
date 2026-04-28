use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{hooks::use_params, params::Params};

use crate::{
    components::{markdown::Markdown, spinner::Spinner},
    pages::reference::{
        RefSidebarEntries, ReferenceFeaturesView, ReferenceSidebar, collect_feature_views,
    },
    rules::{DefinitionStore, IndexEntry, RulesRegistry},
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct BackgroundRefParams {
    name: Option<String>,
}

#[component]
pub fn BackgroundReference() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let params = use_params::<BackgroundRefParams>();

    let bg_name = move || params.get().ok().and_then(|p| p.name).unwrap_or_default();

    Effect::new(move || {
        let name = bg_name();
        if !name.is_empty() {
            registry.backgrounds().fetch(&name);
        }
    });

    let current_label = Signal::derive(move || {
        registry
            .index()
            .entry_label_desc(IndexEntry::Background(&bg_name()))
            .0
            .get()
    });

    let detail = move || {
        let name = bg_name();

        if name.is_empty() {
            return Some(
                view! {
                    <div class="reference-empty">
                        <p>{move_tr!("ref-select-background")}</p>
                    </div>
                }
                .into_any(),
            );
        }

        let (title, description, feature_names) = registry.backgrounds().lookup(&name, |loc| {
            (
                loc.label().to_string(),
                loc.description().to_string(),
                loc.data.features.clone(),
            )
        })?;

        let features = registry.with_features_index(|features_index| {
            let iter = feature_names
                .iter()
                .filter_map(|name| features_index.get(name.as_str()));
            collect_feature_views(iter)
        });

        Some(
            view! {
                <Title text=title.clone() />
                <div class="reference-detail">
                    <h1>{title}</h1>
                    <Markdown text=description />

                    {(!features.is_empty()).then(|| view! {
                        <h2>{move_tr!("ref-features")}</h2>
                        <ReferenceFeaturesView features />
                    })}
                </div>
            }
            .into_any(),
        )
    };

    let loading = Signal::derive(move || {
        let name = bg_name();
        !name.is_empty() && registry.backgrounds().with(&name, |_| ()).is_none()
    });

    view! {
        <Spinner loading />
        <Title text=move_tr!(i18n, "ref-backgrounds") />
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    <RefSidebarEntries
                        names=Signal::derive(move || registry.with_background_entries(|entries| {
                            entries.values().map(|entry| entry.name.to_string()).collect()
                        }))
                        kind=|n| IndexEntry::Background(n)
                    />
                </ReferenceSidebar>
                <main class="reference-main">
                    {detail}
                </main>
            </div>
        </div>
    }
}
