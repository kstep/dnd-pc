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
struct RaceRefParams {
    name: Option<String>,
}

#[component]
pub fn RaceReference() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let params = use_params::<RaceRefParams>();

    let race_name = move || params.get().ok().and_then(|p| p.name).unwrap_or_default();

    Effect::new(move || {
        let name = race_name();
        if !name.is_empty() {
            registry.races().fetch_tracked(&name);
        }
    });

    let current_label = Signal::derive(move || registry.race_label_by_name(&race_name()));

    let detail = move || {
        let name = race_name();

        if name.is_empty() {
            return view! {
                <div class="reference-empty">
                    <p>{move_tr!("ref-select-race")}</p>
                </div>
            }
            .into_any();
        }

        registry
            .races()
            .with_tracked(&name, |def| {
                let title = def.label().to_string();
                let description = def.description.clone();

                let traits: Vec<(String, String, String, String)> = def
                    .traits
                    .values()
                    .map(|trait_def| {
                        let langs = trait_def.languages.join(", ");
                        let assigns = trait_def
                            .assign
                            .as_deref()
                            .map(|a| super::summarize_assignments(a, &i18n))
                            .unwrap_or_default();
                        (
                            trait_def.label().to_string(),
                            trait_def.description.clone(),
                            langs,
                            assigns,
                        )
                    })
                    .collect();

                let features = registry.with_features_index(|features_index| {
                    let resolved: Vec<_> = def
                        .features
                        .iter()
                        .filter_map(|name| features_index.get(name.as_str()))
                        .collect();
                    collect_feature_views(resolved.into_iter())
                });

                view! {
                    <Title text=title.clone() />
                    <div class="reference-detail">
                        <h1>{title}</h1>
                        <p class="reference-description">{description}</p>

                        {(!traits.is_empty()).then(|| view! {
                            <h2>{move_tr!("racial-traits")}</h2>
                            <div class="reference-features">
                                {traits.into_iter().map(|(label, desc, langs, assigns)| {
                                    view! {
                                        <div class="reference-feature">
                                            <h3>{label}</h3>
                                            <p>{desc}</p>
                                            {(!assigns.is_empty()).then(|| view! {
                                                <p class="feature-assignments">{assigns}</p>
                                            })}
                                            {(!langs.is_empty()).then(|| view! {
                                                <p class="feature-languages">
                                                    {move_tr!("ref-languages")}{": "}{langs}
                                                </p>
                                            })}
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        })}

                        {(!features.is_empty()).then(|| view! {
                            <h2>{move_tr!("ref-features")}</h2>
                            <ReferenceFeaturesView features />
                        })}
                    </div>
                }
                .into_any()
            })
            .unwrap_or_else(|| {
                view! { <p class="reference-loading">{move_tr!("ref-loading")}</p> }.into_any()
            })
    };

    view! {
        <Title text=Signal::derive(move || i18n.tr("ref-races")) />
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    {move || registry.with_race_entries(|entries| {
                        entries.values().map(|entry| {
                            let name = entry.name.clone();
                            let label = entry.label().to_string();
                            view! {
                                <A href=format!("{BASE_URL}/r/race/{name}") attr:class="reference-nav-item">
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
