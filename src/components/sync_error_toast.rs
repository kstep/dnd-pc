use leptos::prelude::*;
use leptos_fluent::tr;

use crate::{
    components::toast::Toast,
    storage::{self, SyncStatus},
};

#[component]
pub fn SyncErrorToastTrigger() -> impl IntoView {
    let status = storage::sync_status();
    let last_error = storage::sync_last_error();
    // Remembers whether we've already shown a toast for the current Error
    // session — reset once status leaves Error, so a later failure triggers
    // a new toast.
    let shown_for_current = StoredValue::new(false);

    Effect::new(move |_| {
        if status.get() != SyncStatus::Error {
            shown_for_current.set_value(false);
            return;
        }
        if shown_for_current.get_value() {
            return;
        }
        let error = last_error.get().unwrap_or_default();
        shown_for_current.set_value(true);
        Toast::error(tr!("toast-sync-error", { "error" => error })).show();
    });
}
