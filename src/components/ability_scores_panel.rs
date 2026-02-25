use leptos::prelude::*;
use leptos_fluent::move_tr;
use strum::IntoEnumIterator;

use crate::{
    components::{ability_score_block::AbilityScoreBlock, panel::Panel},
    model::Ability,
};

#[component]
pub fn AbilityScoresPanel() -> impl IntoView {
    view! {
        <Panel title=move_tr!("panel-ability-scores") class="ability-scores-panel">
            <div class="ability-scores-grid">
                {Ability::iter()
                    .map(|ability| view! { <AbilityScoreBlock ability=ability /> })
                    .collect_view()}
            </div>
        </Panel>
    }
}
