use leptos::prelude::*;

use crate::components::{
    character_header::CharacterHeader,
    panels::{
        ability_scores::AbilityScoresPanel, class_fields::ClassFieldsPanels, combat::CombatPanel,
        equipment::EquipmentPanel, features::FeaturesPanel, notes::NotesPanel,
        personality::PersonalityPanel, proficiencies::ProficienciesPanel,
        saving_throws::SavingThrowsPanel, skills::SkillsPanel, spellcasting::SpellcastingPanel,
    },
};

#[component]
pub fn CharacterEditor() -> impl IntoView {
    view! {
        <CharacterHeader />
        <div class="editor-grid">
            <div class="editor-column editor-column-left">
                <AbilityScoresPanel />
                <SavingThrowsPanel />
                <SkillsPanel />
                <ProficienciesPanel />
            </div>
            <div class="editor-column editor-column-center">
                <CombatPanel />
                <EquipmentPanel />
                <NotesPanel />
                <PersonalityPanel />
            </div>
            <div class="editor-column editor-column-right">
                <SpellcastingPanel />
                <ClassFieldsPanels />
                <FeaturesPanel />
            </div>
        </div>
    }
}
