use std::time::Duration;

use gloo_storage::{LocalStorage, Storage};
use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::move_tr;

use crate::{components::toast::Toast, storage};

const DISMISS_KEY: &str = "dnd_pc_signin_toast_dismissed";
const SHOW_DELAY: Duration = Duration::from_secs(2);

#[component]
pub fn SignInToastTrigger() -> impl IntoView {
    let should_prompt = storage::should_prompt_sign_in();
    let shown = StoredValue::new(false);

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
        // Build the toast synchronously in the Effect — tr!() and Toast::new
        // both need the current owner. The Toast captures it so `.show()` can
        // run later from the `set_timeout` callback where no owner is active.
        let toast = Toast::i18n("toast-signin-prompt")
            .persist()
            .with_action(
                move_tr!("toast-signin-action"),
                Callback::new(|_| storage::sign_in_with_google()),
            )
            .on_dismiss(Callback::new(|_| {
                let _ = LocalStorage::set(DISMISS_KEY, true);
            }));
        set_timeout(move || toast.show(), SHOW_DELAY);
    });
}
