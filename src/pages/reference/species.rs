use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, params::Params};

use super::{ReferenceFeaturesView, ReferenceSidebar, collect_feature_views, encode_name};
use crate::{
    BASE_URL,
    components::loader::Loader,
    rules::{DefinitionStore, RulesRegistry},
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct SpeciesRefParams {
    name: Option<String>,
}

#[component]
pub fn SpeciesReference() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let params = use_params::<SpeciesRefParams>();

    let species_name = move || params.get().ok().and_then(|p| p.name).unwrap_or_default();

    Effect::new(move || {
        let name = species_name();
        if !name.is_empty() {
            registry.species().fetch_tracked(&name);
        }
    });

    let current_label = Signal::derive(move || registry.species_label_by_name(&species_name()));

    let detail = move || {
        let name = species_name();

        if name.is_empty() {
            return view! {
                <div class="reference-empty">
                    <p>{move_tr!("ref-select-species")}</p>
                </div>
            }
            .into_any();
        }

        let Some((title, description, feature_names)) =
            registry.species().with_tracked(&name, |def| {
                (
                    def.label().to_string(),
                    def.description.clone(),
                    def.features.clone(),
                )
            })
        else {
            return view! { <Loader /> }.into_any();
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
        <Title text=Signal::derive(move || i18n.tr("ref-species")) />
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    {move || registry.with_species_entries(|entries| {
                        entries.values().map(|entry| {
                            let name = entry.name.clone();
                            let label = entry.label().to_string();
                            let href = format!("{BASE_URL}/r/species/{}", encode_name(&name));
                            view! {
                                <A href=href attr:class="reference-nav-item">
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
