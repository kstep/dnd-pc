use leptos::prelude::*;
use leptos_fluent::move_tr;
use strum::IntoEnumIterator;

use crate::{
    components::{panel::Panel, skill_row::SkillRow},
    model::{Ability, Skill, Translatable},
};

#[component]
pub fn SkillsPanel() -> impl IntoView {
    let groups: Vec<(Ability, Vec<Skill>)> = Ability::iter()
        .map(|ability| {
            let skills: Vec<Skill> = Skill::iter().filter(|s| s.ability() == ability).collect();
            (ability, skills)
        })
        .filter(|(_, skills)| !skills.is_empty())
        .collect();

    let i18n = expect_context::<leptos_fluent::I18n>();

    view! {
        <Panel title=move_tr!("panel-skills") class="skills-panel">
            {groups
                .into_iter()
                .map(|(ability, skills)| {
                    let tr_key = ability.tr_key();
                    let label = Signal::derive(move || i18n.tr(tr_key));
                    view! {
                        <div class="skill-group">
                            <h4 class="skill-group-header">{label}</h4>
                            {skills
                                .into_iter()
                                .map(|skill| view! { <SkillRow skill=skill /> })
                                .collect_view()}
                        </div>
                    }
                })
                .collect_view()}
        </Panel>
    }
}
