mod backpack;
mod choices;
mod languages;
mod resources;
mod spell_slots;
mod spells;
mod stats;
mod weapons;

use backpack::BackpackBlock;
use choices::ChoicesBlock;
use languages::LanguagesBlock;
use leptos::prelude::*;
use leptos_fluent::move_tr;
use resources::ResourcesBlock;
use spell_slots::SpellSlotsBlock;
use spells::SpellsBlock;
use stats::StatsBlock;
use weapons::WeaponsBlock;

use crate::components::summary_header::SummaryHeader;

#[component]
pub fn CharacterSummary() -> impl IntoView {
    view! {
        <SummaryHeader />
        <div class="summary-page">

            <div class="summary-top-row">
            // === Section: What Can I Do? ===
            <div class="summary-section summary-section-actions">
                <h3 class="summary-section-title">{move_tr!("summary-actions")}</h3>
                <WeaponsBlock />
                <SpellsBlock />
                <SpellSlotsBlock />
                <ResourcesBlock />
                <ChoicesBlock />
                <LanguagesBlock />
            </div>

            // === Section: Main Stats ===
            <StatsBlock />
            </div>

            // === Section: Backpack ===
            <BackpackBlock />
        </div>
    }
}
