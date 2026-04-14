use leptos::prelude::*;

use crate::components::panels::{notes::NotesPanel, personality::PersonalityPanel};

#[component]
pub fn BackstoryTab() -> impl IntoView {
    view! {
        <div class="editor-tab">
            <PersonalityPanel />
            <NotesPanel />
        </div>
    }
}
