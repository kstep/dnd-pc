use leptos::prelude::*;
use strum::IntoEnumIterator;

use crate::{components::ability_score_block::AbilityScoreBlock, model::Ability};

#[component]
pub fn AbilityScoresPanel() -> impl IntoView {
    view! {
        <div class="panel ability-scores-panel">
            <h3>"Ability Scores"</h3>
            <div class="ability-scores-grid">
                {Ability::iter()
                    .map(|ability| view! { <AbilityScoreBlock ability=ability /> })
                    .collect_view()}
            </div>
        </div>
    }
}
