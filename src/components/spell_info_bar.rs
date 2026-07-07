use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{
    components::icon::Icon,
    model::{ActionType, EffectDuration, EffectRange, Translatable},
    rules::{CastTime, SpellComponents, SpellMeta},
};

fn format_components(components: SpellComponents) -> Option<impl IntoView> {
    let mut icons: Vec<AnyView> = Vec::new();
    if components.verbal {
        icons.push(
            view! {
                <span title=move_tr!("ref-spell-comp-verbal")>
                    <Icon name="audio-lines" />
                </span>
            }
            .into_any(),
        );
    }
    if components.somatic {
        icons.push(
            view! {
                <span title=move_tr!("ref-spell-comp-somatic")>
                    <Icon name="hand-helping" />
                </span>
            }
            .into_any(),
        );
    }
    let material = components.material.map(|material| {
        let name = material.name.to_string();
        let price = material.price.map(|money| money.to_string());
        let icon = if material.consumable { "gem" } else { "stone" };
        let title = if material.consumable {
            move_tr!("ref-spell-comp-consumable")
        } else {
            move_tr!("ref-spell-comp-material")
        };
        view! {
            <span title=title>
                <Icon name=icon />
            </span>
            {(!name.is_empty()).then(|| view! { <span class="spell-tag">{name}</span> })}
            {price.map(|price| view! { <span class="spell-tag">{price}</span> })}
        }
    });
    if icons.is_empty() && material.is_none() {
        return None;
    }
    Some(view! {
        <div class="info-item">
            <span class="info-label">{move_tr!("ref-spell-components")}</span>
            <span class="info-value spell-comp-value">{icons}{material}</span>
        </div>
    })
}

fn format_cast_time(cast_time: CastTime, ritual: bool) -> impl IntoView {
    let base = match cast_time {
        CastTime::Action(ActionType::Action) => move_tr!("action-type-action"),
        CastTime::Action(ActionType::BonusAction) => move_tr!("action-type-bonus-action"),
        CastTime::Action(ActionType::Reaction) => move_tr!("action-type-reaction"),
        CastTime::Rounds(rounds) => {
            if rounds >= 600 {
                move_tr!("ref-spell-cast-hours", {"hours" => (rounds / 600).to_string()})
            } else if rounds >= 10 {
                move_tr!("ref-spell-cast-minutes", {"minutes" => (rounds / 10).to_string()})
            } else {
                move_tr!("ref-spell-cast-rounds", {"rounds" => rounds.to_string()})
            }
        }
    };
    view! {
        {base}
        {ritual.then(|| view! { <span class="spell-tag">{move_tr!("ref-spell-ritual")}</span> })}
    }
}

fn format_range(range: EffectRange) -> impl IntoView {
    match range {
        EffectRange::Caster => move_tr!("ref-spell-range-self").into_any(),
        EffectRange::Touch => move_tr!("ref-spell-range-touch").into_any(),
        EffectRange::Feet(feet) => {
            move_tr!("ref-spell-range-feet", {"feet" => feet.to_string()}).into_any()
        }
    }
}

fn format_duration(duration: EffectDuration) -> impl IntoView {
    match duration {
        EffectDuration::Instant => move_tr!("ref-spell-duration-instant").into_any(),
        EffectDuration::Forever => move_tr!("ref-spell-duration-forever").into_any(),
        EffectDuration::Rounds(rounds) => {
            if rounds >= 600 {
                move_tr!("ref-spell-duration-hours", {"hours" => (rounds / 600).to_string()})
                    .into_any()
            } else if rounds >= 10 {
                move_tr!("ref-spell-duration-minutes", {"minutes" => (rounds / 10).to_string()})
                    .into_any()
            } else {
                move_tr!("ref-spell-duration-rounds", {"rounds" => rounds}).into_any()
            }
        }
    }
}

#[component]
pub fn SpellInfoBar(
    meta: SpellMeta,
    #[prop(optional)] components: SpellComponents,
) -> impl IntoView {
    let i18n = expect_context::<leptos_fluent::I18n>();
    let category_key = meta.category.tr_key();
    let category_label = Signal::derive(move || i18n.tr(category_key));
    view! {
        <div class="reference-info-bar spell-info-bar entry-sublabel">
            <div class="info-item">
                <span class="info-label">{move_tr!("ref-spell-category")}</span>
                <span class="info-value">{category_label}</span>
            </div>
            <div class="info-item">
                <span class="info-label">{move_tr!("ref-spell-cast-time")}</span>
                <span class="info-value">{format_cast_time(meta.cast_time, meta.ritual)}</span>
            </div>
            {meta
                .range
                .map(|range| {
                    view! {
                        <div class="info-item">
                            <span class="info-label">{move_tr!("ref-spell-range")}</span>
                            <span class="info-value">{format_range(range)}</span>
                        </div>
                    }
                })}
            {meta
                .duration
                .map(|duration| {
                    view! {
                        <div class="info-item">
                            <span class="info-label">{move_tr!("ref-spell-duration")}</span>
                            <span class="info-value">
                                {format_duration(duration)}
                                {meta
                                    .concentration
                                    .then(|| {
                                        view! {
                                            <span class="spell-tag">
                                                {move_tr!("ref-spell-concentration")}
                                            </span>
                                        }
                                    })}
                            </span>
                        </div>
                    }
                })}
            {format_components(components)}
        </div>
    }
}
