use leptos::prelude::*;

use crate::components::icon::Icon;

#[component]
pub fn ToggleButton(
    #[prop(into)] expanded: Signal<bool>,
    on_toggle: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <button
            class="btn-toggle-desc"
            on:click=move |_| on_toggle()
        >
            <Icon name=move || if expanded.get() { "minus" } else { "plus" } />
        </button>
    }
}
