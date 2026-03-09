mod backpack;
mod choices;
mod languages;
mod resources;
mod spells;
mod stats;
mod weapons;

use backpack::BackpackBlock;
use choices::ChoicesBlock;
use languages::LanguagesBlock;
use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use resources::ResourcesBlock;
use spells::SpellsBlock;
use stats::StatsBlock;
use weapons::WeaponsBlock;

use crate::{
    components::{icon::Icon, summary_header::SummaryHeader},
    model::Character,
};

#[component]
pub fn CharacterSummary() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    view! {
        <SummaryHeader />
        <div class="summary-page">

            <div class="summary-top-row">
            // === Section: What Can I Do? ===
            <div class="summary-section summary-section-actions">
                <h3 class="summary-section-title">{move_tr!("summary-actions")}</h3>
                <div class="summary-rest-actions">
                    <button class="summary-rest-btn" title=move_tr!("short-rest")
                        on:click=move |_| { store.update(|ch| ch.short_rest()); }
                    >
                        <Icon name="coffee" size=14 />
                    </button>
                    <button class="summary-rest-btn" title=move_tr!("long-rest")
                        on:click=move |_| { store.update(|ch| ch.long_rest()); }
                    >
                        <Icon name="moon" size=14 />
                    </button>
                </div>
                <WeaponsBlock />
                <SpellsBlock />
                <ChoicesBlock />
                <LanguagesBlock />
            </div>

            // === Right column: Stats + Resources ===
            <div class="summary-right-column">
                <ResourcesBlock />
                <StatsBlock />
            </div>
            </div>

            // === Section: Backpack ===
            <BackpackBlock />
        </div>
    }
}
