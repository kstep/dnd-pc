use leptos::prelude::*;

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
            {move || if expanded.get() { "\u{2212}" } else { "+" }}
        </button>
    }
}
