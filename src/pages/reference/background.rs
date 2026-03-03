use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_meta::Title;
use leptos_router::{components::A, hooks::use_params, params::Params};

use super::{FeatureSpells, FeatureSpellsView};
use crate::{
    BASE_URL,
    model::{Translatable, format_bonus},
    rules::RulesRegistry,
};

#[derive(Params, Clone, Debug, PartialEq)]
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
            registry.fetch_background_tracked(&name);
        }
    });

    let sidebar = move || {
        let home = view! {
            <A href=format!("{BASE_URL}/") attr:class="reference-home-link">
                {"\u{2190} "}{move_tr!("ref-home")}
            </A>
        };
        let entries = registry.with_background_entries(|entries| {
            entries
                .iter()
                .map(|entry| {
                    let name = entry.name.clone();
                    let label = entry.label().to_string();
                    view! {
                        <A href=format!("{BASE_URL}/r/background/{name}") attr:class="reference-nav-item">
                            {label}
                        </A>
                    }
                })
                .collect_view()
        });
        view! { {home} {entries} }
    };

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

        registry
            .with_background_tracked(&name, |def| {
                let title = def.label().to_string();
                let description = def.description.clone();

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

                let skill_profs = def
                    .proficiencies
                    .iter()
                    .map(|s| i18n.tr(s.tr_key()))
                    .collect::<Vec<_>>()
                    .join(", ");

                let features: Vec<(String, String, FeatureSpells)> = def
                    .features
                    .values()
                    .map(|f| {
                        let spells =
                            FeatureSpells::from_spell_list(f.spells.as_ref().map(|s| &s.list));
                        (f.label().to_string(), f.description.clone(), spells)
                    })
                    .collect();

                view! {
                    <Title text=title.clone() />
                    <div class="reference-detail">
                        <h1>{title}</h1>
                        <p class="reference-description">{description}</p>

                        <div class="reference-info-bar">
                            {(!ability_mods.is_empty()).then(|| view! {
                                <div class="info-item">
                                    <span class="info-label">{move_tr!("ref-ability-mods")}</span>
                                    <span class="info-value">{ability_mods}</span>
                                </div>
                            })}
                            {(!skill_profs.is_empty()).then(|| view! {
                                <div class="info-item">
                                    <span class="info-label">{move_tr!("ref-skill-profs")}</span>
                                    <span class="info-value">{skill_profs}</span>
                                </div>
                            })}
                        </div>

                        {(!features.is_empty()).then(|| view! {
                            <h2>{move_tr!("ref-features")}</h2>
                            <div class="reference-features">
                                {features.into_iter().map(|(label, desc, spells)| {
                                    view! {
                                        <div class="reference-feature">
                                            <h3>{label}</h3>
                                            <p>{desc}</p>
                                            <FeatureSpellsView spells=spells />
                                        </div>
                                    }
                                }).collect_view()}
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
        <div class="reference-page">
            <div class="reference-layout">
                <aside class="reference-sidebar">
                    {sidebar}
                </aside>
                <main class="reference-main">
                    {detail}
                </main>
            </div>
        </div>
    }
}
