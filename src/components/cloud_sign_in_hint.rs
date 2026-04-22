use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{components::hint_banner::HintBanner, storage};

#[component]
pub fn CloudSignInHint(#[prop(into)] message: Signal<String>) -> impl IntoView {
    let visible = storage::should_prompt_sign_in();
    view! {
        <HintBanner
            icon="cloud"
            visible=visible
            action_label=move_tr!("hint-sign-in-button")
            on_action=Callback::new(|()| storage::sign_in_with_google())
        >
            <p class="hint-banner-text">{message}</p>
        </HintBanner>
    }
}
