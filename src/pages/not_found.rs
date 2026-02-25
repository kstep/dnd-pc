use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="not-found">
            <h1>"Page not found"</h1>
            <A href=format!("{}/", crate::BASE_URL)>"Back to character list"</A>
        </div>
    }
}
