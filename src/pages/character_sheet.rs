use leptos::prelude::*;
use leptos_router::{components::A, hooks::use_params_map};
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    components::{
        ability_scores_panel::AbilityScoresPanel, character_header::CharacterHeader,
        combat_panel::CombatPanel, equipment_panel::EquipmentPanel, features_panel::FeaturesPanel,
        notes_panel::NotesPanel, personality_panel::PersonalityPanel,
        proficiencies_panel::ProficienciesPanel, saving_throws_panel::SavingThrowsPanel,
        skills_panel::SkillsPanel, spellcasting_panel::SpellcastingPanel,
    },
    storage,
};

#[component]
pub fn CharacterSheet() -> impl IntoView {
    let params = use_params_map();

    let id = move || {
        params
            .read()
            .get("id")
            .and_then(|id| Uuid::parse_str(&id).ok())
    };

    let character = move || id().and_then(|id| storage::load_character(&id));

    // If the character exists, render the sheet; otherwise show not-found
    move || {
        if let Some(char_data) = character() {
            let store = Store::new(char_data);

            // Auto-save effect
            Effect::new(move || {
                let c = store.get();
                storage::save_character(&c);
            });

            // Provide context so child components can access the store
            provide_context(store);

            view! {
                <div class="character-sheet">
                    <CharacterHeader />
                    <div class="sheet-grid">
                        <div class="sheet-column sheet-column-left">
                            <AbilityScoresPanel />
                            <SavingThrowsPanel />
                            <SkillsPanel />
                        </div>
                        <div class="sheet-column sheet-column-center">
                            <CombatPanel />
                            <EquipmentPanel />
                            <NotesPanel />
                            <PersonalityPanel />
                        </div>
                        <div class="sheet-column sheet-column-right">
                            <SpellcastingPanel />
                            <FeaturesPanel />
                            <ProficienciesPanel />
                        </div>
                    </div>
                </div>
            }
            .into_any()
        } else {
            view! {
                <div class="not-found">
                    <h1>"Character not found"</h1>
                    <A href=format!("{}/", crate::BASE_URL)>"Back to character list"</A>
                </div>
            }
            .into_any()
        }
    }
}
