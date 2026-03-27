use std::str::FromStr;

use leptos::{prelude::*, reactive::computed::Memo};
use leptos_router::{
    NavigateOptions,
    hooks::{query_signal_with_options, use_location},
};
use wasm_bindgen::{JsCast, prelude::Closure};

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

/// Returns a function that builds an absolute hash href from the current
/// pathname. Works around `<base data-trunk-public-url />` which causes bare
/// `#hash` links to resolve relative to the base URL instead of the current
/// page.
pub fn use_hash_href() -> impl Fn(&str) -> String {
    let pathname = use_location().pathname;
    move |hash: &str| pathname.with_untracked(|path| format!("{path}#{hash}"))
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
