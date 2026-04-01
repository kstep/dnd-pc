use std::time::Duration;

use leptos::{leptos_dom::helpers::set_timeout_with_handle, prelude::*};

use crate::{
    rules::RulesRegistry,
    storage::{self, SyncStatus},
};

#[derive(Clone, Copy, Default)]
pub struct IsRouting(pub RwSignal<bool>);

const MIN_SPIN_MS: f64 = 500.0;

#[component]
pub fn Logo() -> impl IntoView {
    let registry = expect_context::<RulesRegistry>();
    let is_routing = expect_context::<IsRouting>().0;
    let sync_status = storage::sync_status();
    let is_busy = Memo::new(move |_| {
        registry.is_loading()
            || is_routing.get()
            || matches!(
                sync_status.get(),
                SyncStatus::Connecting | SyncStatus::Syncing
            )
    });

    let spinning = RwSignal::new(false);
    let spin_start = RwSignal::new(0.0_f64);

    Effect::new(move |_| {
        if is_busy.get() {
            spin_start.set(js_sys::Date::now());
            spinning.set(true);
        } else if spinning.get_untracked() {
            let elapsed = js_sys::Date::now() - spin_start.get_untracked();
            let remaining = (MIN_SPIN_MS - elapsed).max(0.0);
            if remaining <= 0.0 {
                spinning.set(false);
            } else {
                let _ = set_timeout_with_handle(
                    move || spinning.set(false),
                    Duration::from_millis(remaining as u64),
                );
            }
        }
    });

    view! {
        <span class="navbar-logo" class:loading=spinning
            inner_html=crate::LOGO_SVG />
    }
}
