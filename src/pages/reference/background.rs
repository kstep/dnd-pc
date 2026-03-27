use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, params::Params};

use super::{ReferenceFeaturesView, ReferenceSidebar, collect_feature_views};
use crate::{
    BASE_URL,
    rules::{DefinitionStore, RulesRegistry},
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
            registry.backgrounds().fetch_tracked(&name);
        }
    });

    let current_label = Signal::derive(move || registry.background_label_by_name(&bg_name()));

    let detail = move || {
        let name = bg_name();

        if name.is_empty() {
            return view! {
                <div class="reference-empty">
                    <p>{move_tr!("ref-select-background")}</p>
                </div>
            }
            .into_any();
        }

        let Some((title, description, feature_names)) =
            registry.backgrounds().with_tracked(&name, |def| {
                (
                    def.label().to_string(),
                    def.description.clone(),
                    def.features.clone(),
                )
            })
        else {
            return view! { <p class="reference-loading">{move_tr!("ref-loading")}</p> }.into_any();
        };

        let features = registry.with_features_index(|features_index| {
            let iter = feature_names
                .iter()
                .filter_map(|name| features_index.get(name.as_str()));
            collect_feature_views(iter)
        });

        view! {
            <Title text=title.clone() />
            <div class="reference-detail">
                <h1>{title}</h1>
                <p class="reference-description">{description}</p>

                {(!features.is_empty()).then(|| view! {
                    <h2>{move_tr!("ref-features")}</h2>
                    <ReferenceFeaturesView features />
                })}
            </div>
        }
        .into_any()
    };

    view! {
        <Title text=Signal::derive(move || i18n.tr("ref-backgrounds")) />
        <div class="reference-layout">
            <ReferenceSidebar current_label>
                {move || registry.with_background_entries(|entries| {
                    entries.values().map(|entry| {
                        let name = entry.name.clone();
                        let label = entry.label().to_string();
                        view! {
                            <A href=format!("{BASE_URL}/r/background/{name}") attr:class="reference-nav-item">
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
    }
}
