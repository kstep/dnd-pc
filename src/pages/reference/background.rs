use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{hooks::use_params, params::Params};

use crate::{
    components::{markdown::Markdown, package_picker::ReferencePackagesBar, spinner::Spinner},
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

    let current_label = Signal::derive(move || {
        registry
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

        let (title, description, feature_names, package) =
            registry.backgrounds().lookup(&name, |loc| {
                (
                    loc.label().to_string(),
                    loc.description().to_string(),
                    loc.data.features.clone(),
                    loc.data.package.to_string(),
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

                    {(!package.is_empty())
                        .then(|| {
                            let package_label = move || registry.package_display_name(&package);
                            view! {
                                <div class="reference-info-bar">
                                    <div class="info-item">
                                        <span class="info-label">{move_tr!("ref-package")}</span>
                                        <span class="info-value">{package_label}</span>
                                    </div>
                                </div>
                            }
                        })}

                    {(!features.is_empty())
                        .then(|| {
                            view! {
                                <h2>{move_tr!("ref-features")}</h2>
                                <ReferenceFeaturesView features />
                            }
                        })}
                </div>
            }
            .into_any(),
        )
    };

    // Defs are eager: spin only while the index itself loads; a name absent
    // from the active package set renders as empty, not an infinite spinner.
    let loading = Signal::derive(move || {
        let name = bg_name();
        !name.is_empty() && registry.backgrounds().index().is_pending()
    });

    view! {
        <Spinner loading />
        <Title text=move_tr!(i18n, "ref-backgrounds") />
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    <RefSidebarEntries
                        names=Signal::derive(move || {
                            registry
                                .with_background_defs(|defs| {
                                    defs.keys().map(|name| name.to_string()).collect()
                                })
                        })
                        kind=|n| IndexEntry::Background(n)
                    />
                </ReferenceSidebar>
                <main class="reference-main">
                    <ReferencePackagesBar />
                    {detail}
                </main>
            </div>
        </div>
    }
}
