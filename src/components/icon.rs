use leptos::prelude::*;

use crate::BASE_URL;

#[component]
pub fn Icon(
    #[prop(into)] name: Signal<&'static str>,
    #[prop(default = 16)] size: u16,
    #[prop(optional, into)] title: Option<String>,
) -> impl IntoView {
    let href = move || format!("{BASE_URL}/icons.svg#icon-{}", name.get());
    let aria_hidden = title.is_none();
    view! {
        <span class="icon-wrapper" title=title>
            <svg class="icon" width=size height=size attr:aria-hidden=aria_hidden>
                <use href=href />
            </svg>
        </span>
    }
}
