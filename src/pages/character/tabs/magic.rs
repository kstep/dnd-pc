use leptos::prelude::*;

use crate::components::panels::spellcasting::SpellcastingPanel;

#[component]
pub fn MagicTab() -> impl IntoView {
    view! {
        <div class="editor-tab">
            <SpellcastingPanel />
        </div>
    }
}
