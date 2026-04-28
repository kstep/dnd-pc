use std::{cell::Cell, time::Duration};

use leptos::{leptos_dom::helpers::set_timeout, prelude::*, task::spawn_local};
use leptos_fluent::move_tr;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
    Event, RegistrationOptions, ServiceWorker, ServiceWorkerRegistration, ServiceWorkerState,
    ServiceWorkerUpdateViaCache,
};

use crate::{BASE_URL, components::toast::Toast};

/// Waiting-worker signal; `None` until a SW update is detected.
pub type UpdateSignal = RwSignal<Option<ServiceWorker>>;

thread_local! {
    /// Set when the user clicks "Reload" in the update toast. Gates the
    /// controllerchange handler and the fallback timeout.
    static PENDING_RELOAD: Cell<bool> = const { Cell::new(false) };
}

/// Time to wait for `controllerchange` after `SKIP_WAITING` before forcing a
/// reload anyway — guards against browsers that don't fire the event.
const RELOAD_FALLBACK: Duration = Duration::from_secs(3);

fn force_reload() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

/// Register sw.js, hook lifecycle events, and provide [`UpdateSignal`] in
/// context.
pub fn init_update_listener() {
    let Some(window) = web_sys::window() else {
        return;
    };
    if !window.is_secure_context() {
        log::warn!("Service worker not registered: insecure context");
        return;
    }

    let signal: UpdateSignal = RwSignal::new(None);
    provide_context(signal);

    let sw_container = window.navigator().service_worker();

    // controllerchange fires on first install (clientsClaim) and on takeover
    // after skipWaiting. Only reload when the user explicitly asked for it.
    let on_controller_change = Closure::<dyn FnMut(Event)>::new(move |_| {
        if PENDING_RELOAD.with(Cell::get) {
            force_reload();
        }
    });
    let _ = sw_container.add_event_listener_with_callback(
        "controllerchange",
        on_controller_change.as_ref().unchecked_ref(),
    );
    on_controller_change.forget();

    // updateViaCache: 'none' bypasses HTTP cache when refetching sw.js.
    let opts = RegistrationOptions::new();
    opts.set_update_via_cache(ServiceWorkerUpdateViaCache::None);

    let sw_path = format!("{BASE_URL}/sw.js");
    let promise = sw_container.register_with_options(&sw_path, &opts);

    spawn_local(async move {
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(reg_value) => {
                let registration: ServiceWorkerRegistration = reg_value.unchecked_into();
                watch_for_updates(&registration, signal);
                attach_visibility_poll(&registration);
                log::info!("Service worker registration initiated");
            }
            Err(err) => {
                log::warn!("Service worker registration failed: {err:?}");
            }
        }
    });
}

fn watch_for_updates(registration: &ServiceWorkerRegistration, signal: UpdateSignal) {
    let reg_for_closure = registration.clone();
    let on_update_found = Closure::<dyn FnMut(Event)>::new(move |_| {
        let Some(installing) = reg_for_closure.installing() else {
            return;
        };
        let on_state_change = Closure::<dyn FnMut(Event)>::new({
            let installing = installing.clone();
            move |_| {
                if installing.state() != ServiceWorkerState::Installed {
                    return;
                }
                // First install on this client — nothing to update from.
                let has_controller = web_sys::window()
                    .map(|w| w.navigator().service_worker().controller().is_some())
                    .unwrap_or(false);
                if has_controller {
                    signal.set(Some(installing.clone()));
                }
            }
        });
        installing.set_onstatechange(Some(on_state_change.as_ref().unchecked_ref()));
        on_state_change.forget();
    });
    let _ = registration
        .add_event_listener_with_callback("updatefound", on_update_found.as_ref().unchecked_ref());
    on_update_found.forget();
}

fn attach_visibility_poll(registration: &ServiceWorkerRegistration) {
    let registration = registration.clone();
    let on_visibility = Closure::<dyn FnMut(Event)>::new(move |_| {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        if !document.hidden() {
            let _ = registration.update();
        }
    });
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        let _ = document.add_event_listener_with_callback(
            "visibilitychange",
            on_visibility.as_ref().unchecked_ref(),
        );
    }
    on_visibility.forget();
}

#[component]
pub fn UpdateToastTrigger() -> impl IntoView {
    let Some(signal) = use_context::<UpdateSignal>() else {
        return;
    };

    // Build the toast once at component scope (App lifetime). The reload
    // callback needs the `worker` resolved by the Effect, so we pass it into
    // a `StoredValue` slot the callback reads at click time.
    let pending_worker = StoredValue::<Option<ServiceWorker>>::new(None);
    let toast = Toast::i18n("update-available").persist().with_action(
        move_tr!("update-button-reload"),
        Callback::new(move |_| {
            let Some(worker) = pending_worker.try_update_value(Option::take).flatten() else {
                return;
            };
            PENDING_RELOAD.with(|c| c.set(true));
            let msg = js_sys::Object::new();
            let _ = js_sys::Reflect::set(
                &msg,
                &JsValue::from_str("type"),
                &JsValue::from_str("SKIP_WAITING"),
            );
            let _ = worker.post_message(&msg.into());
            // Plan B: SW didn't fire controllerchange — force reload.
            set_timeout(
                || {
                    if PENDING_RELOAD.with(Cell::get) {
                        force_reload();
                    }
                },
                RELOAD_FALLBACK,
            );
        }),
    );
    let toast = StoredValue::new(Some(toast));

    Effect::new(move |_| {
        let Some(worker) = signal.get() else {
            return;
        };
        pending_worker.set_value(Some(worker));
        let Some(toast) = toast.try_update_value(Option::take).flatten() else {
            return;
        };
        toast.show();
    });
}
