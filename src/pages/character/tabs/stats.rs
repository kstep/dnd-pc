use leptos::prelude::*;

use crate::components::panels::{
    ability_scores::AbilityScoresPanel, combat::CombatPanel,
    damage_modifiers::DamageModifiersPanel, proficiencies::ProficienciesPanel, senses::SensesPanel,
    skills::SkillsPanel,
};

#[component]
pub fn StatsTab() -> impl IntoView {
    view! {
        <div class="editor-tab">
            <AbilityScoresPanel />
            <DamageModifiersPanel />
            <SensesPanel />
            <CombatPanel />
            <SkillsPanel />
            <ProficienciesPanel />
        </div>
    }
}
