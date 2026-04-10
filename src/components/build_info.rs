use leptos::prelude::*;

use crate::{BUILD_COMMIT, BUILD_DATE};

#[component]
pub fn BuildInfo() -> impl IntoView {
    view! {
        <div style="color:transparent;user-select:all;margin-top:1rem;margin-bottom:5rem">
            "build: " {BUILD_COMMIT} " " {BUILD_DATE}
        </div>
    }
}
