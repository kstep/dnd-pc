use leptos::prelude::*;

use crate::LOGO_SVG;

#[component]
pub fn Loader() -> impl IntoView {
    view! {
        <div class="loader" inner_html=LOGO_SVG />
    }
}
