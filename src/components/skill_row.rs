use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::{icon::Icon, stat_box::StatBox},
    model::{
        Character, CharacterCoreStoreFields, CharacterStoreFields, ProficiencyLevel, Skill,
        Translatable, format_bonus,
    },
};

#[component]
pub fn SkillRow(skill: Skill) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let prof_level = Memo::new(move |_| store.core().skills().read().get(skill));

    let bonus = Memo::new(move |_| store.read().skill_bonus(skill));
    let bonus_display = move || format_bonus(bonus.get());

    let skill_tr_key = skill.tr_key();
    let i18n = expect_context::<leptos_fluent::I18n>();
    let skill_label = Signal::derive(move || i18n.tr(skill_tr_key));

    let highlighted = Signal::derive(move || prof_level.get() != ProficiencyLevel::None);

    view! {
        <StatBox
            label=skill_label
            highlighted=highlighted
            class:clickable=true
            class:glow=move || prof_level.get() == ProficiencyLevel::Expertise
            on:click=move |_| {
                store.core().skills().update(|skills| skills.cycle(skill));
            }
        >
            <span class="stat-highlight">{bonus_display}</span>
            <Icon name=Signal::derive(move || prof_level.get().icon_name()) />
        </StatBox>
    }
}
