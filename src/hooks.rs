use std::str::FromStr;

use leptos::{prelude::*, reactive::computed::Memo};
use leptos_router::{NavigateOptions, hooks::query_signal_with_options};
use wasm_bindgen::{JsCast, prelude::Closure};

pub fn use_debounce(delay_ms: i32) -> impl Fn(Box<dyn FnOnce()>) {
    let handle = RwSignal::new(0i32);
    move |action: Box<dyn FnOnce()>| {
        let window = window();
        let prev = handle.get_untracked();
        if prev != 0 {
            window.clear_timeout_with_handle(prev);
        }
        let callback = Closure::once(move || {
            action();
            handle.set(0);
        });
        match window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            delay_ms,
        ) {
            Ok(id) => handle.set(id),
            Err(error) => log::warn!("set_timeout failed: {error:?}"),
        }
        callback.forget();
    }
}

/// Like `query_signal`, but works correctly with a non-root base URL.
pub fn use_query_signal<T>(
    key: impl Into<leptos::oco::Oco<'static, str>>,
) -> (Memo<Option<T>>, SignalSetter<Option<T>>)
where
    T: FromStr + ToString + PartialEq + Send + Sync,
{
    query_signal_with_options(
        key,
        NavigateOptions {
            resolve: false,
            ..Default::default()
        },
    )
}

/// Returns a reactive signal that tracks the current theme name.
/// Seeds from `window.matchMedia("(prefers-color-scheme: dark)")` and
/// updates in real time via a `change` event listener.
pub fn use_theme() -> ReadSignal<&'static str> {
    let mql = leptos::prelude::window()
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten();
    let theme = RwSignal::new(if mql.as_ref().map(|m| m.matches()).unwrap_or(false) {
        "dark"
    } else {
        "light"
    });
    if let Some(mql) = mql {
        let closure = Closure::<dyn Fn()>::new({
            let mql = mql.clone();
            move || theme.set(if mql.matches() { "dark" } else { "light" })
        });
        let _ = mql.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        closure.into_js_value();
    }
    theme.read_only()
}
