use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::components::A;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="not-found">
            <h1>{move_tr!("page-not-found")}</h1>
            <A href=format!("{}/", crate::BASE_URL)>{move_tr!("back-to-list")}</A>
        </div>
    }
}
