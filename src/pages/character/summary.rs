use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        icon::Icon,
        summary::{
            BackpackBlock, ChoicesBlock, EffectsBlock, LanguagesBlock, ResourcesBlock, SpellsBlock,
            StatsBlock, WeaponsBlock,
        },
        summary_header::SummaryHeader,
        summary_nav::SummaryNav,
    },
    model::Character,
    rules::RulesRegistry,
};

#[component]
pub fn CharacterSummary() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    view! {
        <SummaryHeader />
        <div class="summary-page">

            <div class="summary-top-row">
            // === Section: What Can I Do? ===
            <div class="summary-section summary-section-actions" id="summary-actions">
                <h3 class="summary-section-title">{move_tr!("summary-actions")}</h3>
                <div class="summary-rest-actions">
                    <button class="summary-rest-btn" title=move_tr!("short-rest")
                        on:click=move |_| store.update(|ch| registry.short_rest(ch))
                    >
                        <Icon name="coffee" size=14 />
                    </button>
                    <button class="summary-rest-btn" title=move_tr!("long-rest")
                        on:click=move |_| store.update(|ch| registry.long_rest(ch))
                    >
                        <Icon name="moon" size=14 />
                    </button>
                </div>
                <WeaponsBlock />
                <SpellsBlock />
                <ChoicesBlock />
                <LanguagesBlock />
            </div>

            // === Right column: Effects + Stats + Resources ===
            <div class="summary-right-column">
                <EffectsBlock />
                <ResourcesBlock />
                <StatsBlock />
            </div>
            </div>

            // === Section: Backpack ===
            <BackpackBlock />

            <SummaryNav />
        </div>
    }
}
