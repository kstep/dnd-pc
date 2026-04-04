use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        icon::Icon,
        session::{
            BackpackBlock, ChoicesBlock, DamageModifiersBlock, EffectsBlock, LanguagesBlock,
            ResourcesBlock, SpellsBlock, StatsBlock, WeaponsBlock,
        },
        session_header::SessionHeader,
        session_nav::SessionNav,
    },
    model::Character,
    rules::RulesRegistry,
};

#[component]
pub fn CharacterSession() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let registry = expect_context::<RulesRegistry>();

    view! {
        <SessionHeader />
        <div class="session-page">

            <div class="session-top-row">
            // === Section: What Can I Do? ===
            <div class="session-section session-section-actions" id="session-actions">
                <h3 class="session-section-title">{move_tr!("session-actions")}</h3>
                <div class="session-rest-actions">
                    <button class="session-rest-btn" title=move_tr!("short-rest")
                        on:click=move |_| store.update(|ch| registry.short_rest(ch))
                    >
                        <Icon name="coffee" size=14 />
                    </button>
                    <button class="session-rest-btn" title=move_tr!("long-rest")
                        on:click=move |_| store.update(|ch| registry.long_rest(ch))
                    >
                        <Icon name="moon" size=14 />
                    </button>
                </div>
                <WeaponsBlock />
                <SpellsBlock />
                <ChoicesBlock />
                <LanguagesBlock />
                <DamageModifiersBlock />
            </div>

            // === Right column: Effects + Stats + Resources ===
            <div class="session-right-column">
                <EffectsBlock />
                <ResourcesBlock />
                <StatsBlock />
            </div>
            </div>

            // === Section: Backpack ===
            <BackpackBlock />

            <SessionNav />
        </div>
    }
}
