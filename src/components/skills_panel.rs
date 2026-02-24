use leptos::prelude::*;
use strum::IntoEnumIterator;

use crate::{
    components::skill_row::SkillRow,
    model::{Ability, Skill},
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

    view! {
        <div class="panel skills-panel">
            <h3>"Skills"</h3>
            {groups
                .into_iter()
                .map(|(ability, skills)| {
                    view! {
                        <div class="skill-group">
                            <h4 class="skill-group-header">{ability.to_string()}</h4>
                            {skills
                                .into_iter()
                                .map(|skill| view! { <SkillRow skill=skill /> })
                                .collect_view()}
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}
