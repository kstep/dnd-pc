use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{
    model::{EffectDuration, EffectRange},
    rules::{ActionType, CastTime, SpellMeta},
};

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
        {ritual.then(|| view! {
            <span class="spell-tag">{move_tr!("ref-spell-ritual")}</span>
        })}
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
pub fn SpellInfoBar(meta: SpellMeta) -> impl IntoView {
    view! {
        <div class="reference-info-bar spell-info-bar entry-sublabel">
            <div class="info-item">
                <span class="info-label">{move_tr!("ref-spell-cast-time")}</span>
                <span class="info-value">{format_cast_time(meta.cast_time, meta.ritual)}</span>
            </div>
            {meta.range.map(|range| view! {
                <div class="info-item">
                    <span class="info-label">{move_tr!("ref-spell-range")}</span>
                    <span class="info-value">{format_range(range)}</span>
                </div>
            })}
            {meta.duration.map(|duration| view! {
                <div class="info-item">
                    <span class="info-label">{move_tr!("ref-spell-duration")}</span>
                    <span class="info-value">
                        {format_duration(duration)}
                        {meta.concentration.then(|| view! {
                            <span class="spell-tag">{move_tr!("ref-spell-concentration")}</span>
                        })}
                    </span>
                </div>
            })}
        </div>
    }
}
