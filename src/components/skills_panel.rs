use leptos::prelude::*;
use strum::IntoEnumIterator;

use crate::{components::skill_row::SkillRow, model::Skill};

#[component]
pub fn SkillsPanel() -> impl IntoView {
    view! {
        <div class="panel skills-panel">
            <h3>"Skills"</h3>
            {Skill::iter()
                .map(|skill| view! { <SkillRow skill=skill /> })
                .collect_view()}
        </div>
    }
}
