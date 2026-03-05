use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, params::Params};

use super::{
    FeatureChoicesView, FeatureSpells, FeatureSpellsView, ReferenceSidebar, feature_choices,
};
use crate::{
    BASE_URL,
    model::{Translatable, format_bonus},
    rules::RulesRegistry,
};

#[derive(Params, Clone, Debug, PartialEq)]
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
            registry.fetch_race_tracked(&name);
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
            .with_race_tracked(&name, |def| {
                let title = def.label().to_string();
                let description = def.description.clone();

                let speed = def.speed;
                let ability_mods = def
                    .ability_modifiers
                    .iter()
                    .map(|am| {
                        format!(
                            "{} {}",
                            i18n.tr(am.ability.tr_key()),
                            format_bonus(am.modifier)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                let traits: Vec<(String, String, String)> = def
                    .traits
                    .values()
                    .map(|trait_def| {
                        let langs = trait_def.languages.join(", ");
                        (
                            trait_def.label().to_string(),
                            trait_def.description.clone(),
                            langs,
                        )
                    })
                    .collect();

                let features: Vec<_> = def
                    .features
                    .values()
                    .map(|feat| {
                        let spells = FeatureSpells::from_spell_list(
                            feat.spells.as_ref().map(|spells_def| &spells_def.list),
                        );
                        let choices = feature_choices(&feat.fields);
                        let langs = feat.languages.join(", ");
                        (
                            feat.label().to_string(),
                            feat.description.clone(),
                            langs,
                            spells,
                            choices,
                        )
                    })
                    .collect();

                view! {
                    <Title text=title.clone() />
                    <div class="reference-detail">
                        <h1>{title}</h1>
                        <p class="reference-description">{description}</p>

                        <div class="reference-info-bar">
                            {(speed > 0).then(|| view! {
                                <div class="info-item">
                                    <span class="info-label">{move_tr!("speed")}</span>
                                    <span class="info-value">{format!("{speed} ft")}</span>
                                </div>
                            })}
                            {(!ability_mods.is_empty()).then(|| view! {
                                <div class="info-item">
                                    <span class="info-label">{move_tr!("ref-ability-mods")}</span>
                                    <span class="info-value">{ability_mods}</span>
                                </div>
                            })}
                        </div>

                        {(!traits.is_empty()).then(|| view! {
                            <h2>{move_tr!("racial-traits")}</h2>
                            <div class="reference-features">
                                {traits.into_iter().map(|(label, desc, langs)| {
                                    view! {
                                        <div class="reference-feature">
                                            <h3>{label}</h3>
                                            <p>{desc}</p>
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
                            <div class="reference-features">
                                {features
                                    .into_iter()
                                    .map(|(label, desc, langs, spells, choices)| {
                                        view! {
                                            <div class="reference-feature">
                                                <h3>{label}</h3>
                                                <p>{desc}</p>
                                                {(!langs.is_empty()).then(|| view! {
                                                    <p class="feature-languages">
                                                        {move_tr!("ref-languages")}{": "}{langs}
                                                    </p>
                                                })}
                                                <FeatureSpellsView spells=spells />
                                                <FeatureChoicesView choices=choices />
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                            </div>
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
        <Title text=move_tr!("ref-races") />
        <div class="reference-page">
            <div class="reference-layout">
                <ReferenceSidebar current_label>
                    {move || registry.with_race_entries(|entries| {
                        entries.iter().map(|entry| {
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
