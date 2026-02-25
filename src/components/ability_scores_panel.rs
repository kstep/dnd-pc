use leptos::prelude::*;
use strum::IntoEnumIterator;

use crate::{
    components::{ability_score_block::AbilityScoreBlock, panel::Panel},
    model::Ability,
};

#[component]
pub fn AbilityScoresPanel() -> impl IntoView {
    view! {
        <Panel title="Ability Scores" class="ability-scores-panel">
            <div class="ability-scores-grid">
                {Ability::iter()
                    .map(|ability| view! { <AbilityScoreBlock ability=ability /> })
                    .collect_view()}
            </div>
        </Panel>
    }
}
