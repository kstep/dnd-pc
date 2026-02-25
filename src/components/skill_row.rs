use leptos::prelude::*;
use reactive_stores::Store;

use crate::model::{Character, CharacterStoreFields, ProficiencyLevel, Skill, Translatable};

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

    let skill_tr_key = skill.tr_key();
    let ability_abbr_key = skill.ability().tr_abbr_key();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let skill_label = Signal::derive(move || i18n.tr(skill_tr_key));
    let ability_abbr = Signal::derive(move || i18n.tr(ability_abbr_key));

    view! {
        <div class="skill-row">
            <button
                class="prof-toggle"
                on:click=move |_| {
                    store.skills().update(|skills| {
                        let current = skills.get(&skill).copied().unwrap_or(ProficiencyLevel::None);
                        let next = current.next();
                        if next == ProficiencyLevel::None {
                            skills.remove(&skill);
                        } else {
                            skills.insert(skill, next);
                        }
                    });
                }
            >
                {move || prof_level.get().symbol()}
            </button>
            <span class="skill-bonus">{bonus_display}</span>
            <span class="skill-name">{skill_label}</span>
            <span class="skill-ability">"(" {ability_abbr} ")"</span>
        </div>
    }
}
