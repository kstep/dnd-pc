use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{components::icon::Icon, storage};

#[component]
pub fn CloudSignInHint(#[prop(into)] message: Signal<String>) -> impl IntoView {
    let visible = storage::should_prompt_sign_in();
    view! {
        <Show when=move || visible.get()>
            <div class="cloud-sign-in-hint">
                <Icon name="cloud" size=24 />
                <p class="cloud-sign-in-hint-text">{message}</p>
                <button
                    class="cloud-sign-in-hint-btn"
                    on:click=move |_| storage::sign_in_with_google()
                >
                    {move_tr!("hint-sign-in-button")}
                </button>
            </div>
        </Show>
    }
}
