use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::components::ref_link::Ref;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="not-found">
            <h1>{move_tr!("page-not-found")}</h1>
            <Ref href="/">{move_tr!("back-to-list")}</Ref>
        </div>
    }
}
