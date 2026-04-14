use leptos::prelude::*;
use leptos_fluent::move_tr;
use strum::IntoEnumIterator;

use crate::{
    components::{save_row::SaveRow, skill_row::SkillRow},
    model::{Ability, Skill, Translatable},
};

#[component]
pub fn SkillsPanel() -> impl IntoView {
    let groups: Vec<(Ability, Vec<Skill>)> = Ability::iter()
        .map(|ability| {
            let skills: Vec<Skill> = Skill::iter().filter(|s| s.ability() == ability).collect();
            (ability, skills)
        })
        .collect();

    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <section>
            <h3>{move_tr!("panel-skills")}</h3>
            {groups
                .into_iter()
                .map(|(ability, skills)| {
                    let tr_key = ability.tr_key();
                    let label = Signal::derive(move || i18n.tr(tr_key));
                    view! {
                        <h4>{label}</h4>
                        <div class="slot-box-list">
                            <SaveRow ability=ability />
                            {skills
                                .into_iter()
                                .map(|skill| view! { <SkillRow skill=skill /> })
                                .collect_view()}
                        </div>
                    }
                })
                .collect_view()}
        </section>
    }
}
