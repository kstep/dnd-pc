use leptos::prelude::*;

use crate::components::panels::features::FeaturesPanel;

#[component]
pub fn BuildTab() -> impl IntoView {
    view! {
        <div class="editor-tab">
            <FeaturesPanel />
        </div>
    }
}
