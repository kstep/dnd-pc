use std::time::Duration;

use leptos::prelude::*;

use crate::LOGO_SVG;

#[component]
pub fn Spinner(#[prop(into)] loading: Signal<bool>) -> impl IntoView {
    view! {
        <AnimatedShow
            when=loading
            show_class="fade-in"
            hide_class="fade-out"
            hide_delay=Duration::from_millis(500)
        >
            <div class="spinner" inner_html=LOGO_SVG />
        </AnimatedShow>
    }
}
