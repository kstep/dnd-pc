use std::time::Duration;

use gloo_storage::{LocalStorage, Storage};
use leptos::{leptos_dom::helpers::set_timeout, prelude::*, reactive::owner::Owner};
use leptos_fluent::tr;

use crate::{
    components::toast::{Toast, ToastAction, next_toast_id, show_toast},
    storage,
};

const DISMISS_KEY: &str = "dnd_pc_signin_toast_dismissed";
const SHOW_DELAY: Duration = Duration::from_secs(2);

#[component]
pub fn SignInToastTrigger() -> impl IntoView {
    let should_prompt = storage::should_prompt_sign_in();
    let shown = StoredValue::new(false);
    let owner = Owner::current().expect("SignInToastTrigger needs an owner");

    Effect::new(move |_| {
        if shown.get_value() || !should_prompt.get() {
            return;
        }
        if LocalStorage::get::<bool>(DISMISS_KEY).unwrap_or(false) {
            shown.set_value(true);
            return;
        }
        if storage::load_all_summaries().is_empty() {
            return;
        }
        shown.set_value(true);
        let owner = owner.clone();
        set_timeout(move || owner.with(spawn_signin_toast), SHOW_DELAY);
    });
}

fn spawn_signin_toast() {
    let action = ToastAction {
        label: tr!("toast-signin-action"),
        on_click: Callback::new(|_| storage::sign_in_with_google()),
    };
    show_toast(Toast {
        id: next_toast_id(),
        message: tr!("toast-signin-prompt"),
        action: Some(action),
        auto_close: None,
        on_dismiss: Some(Callback::new(|_| {
            let _ = LocalStorage::set(DISMISS_KEY, true);
        })),
    });
}
