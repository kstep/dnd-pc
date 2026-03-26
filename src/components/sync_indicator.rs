use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{
    components::icon::Icon,
    storage::{self, SyncStatus},
};

#[component]
pub fn SyncIndicator() -> impl IntoView {
    let status = storage::sync_status();
    let is_anonymous = storage::sync_is_anonymous();
    let last_error = storage::sync_last_error();

    let dot_class = move || match status.get() {
        SyncStatus::Disabled => "sync-dot sync-disabled",
        SyncStatus::Connecting => "sync-dot sync-connecting",
        SyncStatus::Synced => "sync-dot sync-synced",
        SyncStatus::Syncing => "sync-dot sync-syncing",
        SyncStatus::Error => "sync-dot sync-error",
    };

    let tr_disabled = move_tr!("sync-disabled");
    let tr_connecting = move_tr!("sync-connecting");
    let tr_synced = move_tr!("sync-synced");
    let tr_syncing = move_tr!("sync-syncing");
    let tr_error = move_tr!("sync-error");

    let status_text = move || match status.get() {
        SyncStatus::Disabled => tr_disabled.get(),
        SyncStatus::Connecting => tr_connecting.get(),
        SyncStatus::Synced => tr_synced.get(),
        SyncStatus::Syncing => tr_syncing.get(),
        SyncStatus::Error => tr_error.get(),
    };

    let show_google_btn = Memo::new(move |_| {
        let current_status = status.get();
        is_anonymous.get()
            && current_status != SyncStatus::Disabled
            && current_status != SyncStatus::Connecting
    });

    let show_retry_btn = Memo::new(move |_| status.get() == SyncStatus::Error);

    let tooltip = move || {
        let base = status_text();
        match last_error.get() {
            Some(err) => format!("{base}: {err}"),
            None => base,
        }
    };

    let tr_sign_in = move_tr!("sync-sign-in-google");

    view! {
        <div class="sync-indicator" title=tooltip>
            <span class=dot_class></span>
            <Show when=move || show_retry_btn.get()>
                <button
                    class="sync-retry-btn"
                    title="Retry"
                    on:click=move |_| storage::retry_sync()
                >
                    <Icon name="refresh-cw" size=14 />
                </button>
            </Show>
            <Show when=move || show_google_btn.get()>
                <button
                    class="sync-google-btn"
                    title=move || tr_sign_in.get()
                    on:click=move |_| storage::sign_in_with_google()
                >
                    <Icon name="log-in" size=14 />
                </button>
            </Show>
        </div>
    }
}
