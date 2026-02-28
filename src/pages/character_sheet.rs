use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::{components::A, hooks::use_params, params::Params};
use reactive_stores::Store;
use uuid::Uuid;

use crate::{
    BASE_URL,
    components::{
        ability_scores_panel::AbilityScoresPanel, character_header::CharacterHeader,
        class_fields_panel::ClassFieldsPanels, combat_panel::CombatPanel,
        equipment_panel::EquipmentPanel, features_panel::FeaturesPanel, notes_panel::NotesPanel,
        personality_panel::PersonalityPanel, proficiencies_panel::ProficienciesPanel,
        saving_throws_panel::SavingThrowsPanel, skills_panel::SkillsPanel,
        spellcasting_panel::SpellcastingPanel,
    },
    rules::RulesRegistry,
    storage,
};

#[derive(Params, Clone, Debug, PartialEq)]
struct CharacterSheetParams {
    id: Uuid,
}

#[component]
pub fn CharacterSheet() -> impl IntoView {
    let params = use_params::<CharacterSheetParams>();

    let id = move || params.get().ok().map(|p| p.id);

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

            // Fill empty descriptions from registry definitions
            let registry = expect_context::<RulesRegistry>();
            {
                let c = store.get_untracked();
                for cl in &c.identity.classes {
                    registry.fetch_class(&cl.class);
                }
                registry.fetch_race(&c.identity.race);
                registry.fetch_background(&c.identity.background);
            }
            Effect::new(move || {
                store.update(|c| {
                    registry.fill_descriptions(c);
                });
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
                            <ProficienciesPanel />
                        </div>
                        <div class="sheet-column sheet-column-center">
                            <CombatPanel />
                            <EquipmentPanel />
                            <NotesPanel />
                            <PersonalityPanel />
                        </div>
                        <div class="sheet-column sheet-column-right">
                            <SpellcastingPanel />
                            <ClassFieldsPanels />
                            <FeaturesPanel />
                        </div>
                    </div>
                </div>
            }
            .into_any()
        } else {
            view! {
                <div class="not-found">
                    <h1>{move_tr!("character-not-found")}</h1>
                    <A href=format!("{BASE_URL}/")>{move_tr!("back-to-list")}</A>
                </div>
            }
            .into_any()
        }
    }
}
