use leptos::prelude::*;
use leptos_fluent::move_tr;
use strum::IntoEnumIterator;

use crate::{components::ability_score_block::AbilityScoreBlock, model::Ability};

#[component]
pub fn AbilityScoresPanel() -> impl IntoView {
    view! {
        <section>
            <h3>{move_tr!("panel-ability-scores")}</h3>
            <div class="slot-box-list">
                {Ability::iter()
                    .map(|ability| view! { <AbilityScoreBlock ability=ability /> })
                    .collect_view()}
            </div>
        </section>
    }
}
