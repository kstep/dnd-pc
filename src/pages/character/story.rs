use leptos::prelude::*;
use leptos_fluent::move_tr;

#[component]
pub fn CharacterStory() -> impl IntoView {
    view! {
        <div class="reference-page">
            <div class="reference-layout">
                <aside class="reference-sidebar">
                    <p>{move_tr!("story-empty")}</p>
                </aside>
                <main class="reference-main">
                    <p>{move_tr!("story-select")}</p>
                </main>
            </div>
        </div>
    }
}
