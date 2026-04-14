use leptos::prelude::*;

#[component]
pub fn StatBox(
    #[prop(into)] label: Signal<String>,
    #[prop(optional, into)] highlighted: Signal<bool>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="stat-box" class:highlighted=move || highlighted.get()>
            <span class="stat-box-label">{label}</span>
            <span class="stat-box-value">{children()}</span>
        </div>
    }
}
