use leptos::prelude::*;

use crate::BASE_URL;

#[component]
pub fn Icon(
    #[prop(into)] name: Signal<&'static str>,
    #[prop(default = 16)] size: u16,
) -> impl IntoView {
    let href = move || format!("{BASE_URL}/icons.svg#icon-{}", name.get());
    let size = size.to_string();
    view! {
        <svg class="icon" width=size.clone() height=size attr:aria-hidden="true">
            <use href=href />
        </svg>
    }
}
