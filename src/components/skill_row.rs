use leptos::prelude::*;
use reactive_stores::Store;

use crate::model::{Character, CharacterStoreFields, ProficiencyLevel, Skill};

#[component]
pub fn SkillRow(skill: Skill) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let prof_level = Memo::new(move |_| {
        store
            .skills()
            .read()
            .get(&skill)
            .copied()
            .unwrap_or(ProficiencyLevel::None)
    });

    let bonus = Memo::new(move |_| store.get().skill_bonus(skill));

    let bonus_display = move || {
        let b = bonus.get();
        if b >= 0 {
            format!("+{b}")
        } else {
            format!("{b}")
        }
    };

    let ability_abbr = match skill.ability() {
        crate::model::Ability::Strength => "STR",
        crate::model::Ability::Dexterity => "DEX",
        crate::model::Ability::Constitution => "CON",
        crate::model::Ability::Intelligence => "INT",
        crate::model::Ability::Wisdom => "WIS",
        crate::model::Ability::Charisma => "CHA",
    };

    view! {
        <div class="skill-row">
            <button
                class="prof-toggle"
                on:click=move |_| {
                    store.skills().update(|skills| {
                        let entry = skills.entry(skill).or_insert(ProficiencyLevel::None);
                        *entry = entry.next();
                    });
                }
            >
                {move || prof_level.get().symbol()}
            </button>
            <span class="skill-bonus">{bonus_display}</span>
            <span class="skill-name">{skill.to_string()}</span>
            <span class="skill-ability">"(" {ability_abbr} ")"</span>
        </div>
    }
}
