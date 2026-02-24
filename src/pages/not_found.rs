use leptos::prelude::*;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="not-found">
            <h1>"Page not found"</h1>
            <a href="/">"Back to character list"</a>
        </div>
    }
}
