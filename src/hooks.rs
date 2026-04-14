use std::{str::FromStr, time::Duration};

use js_sys::Promise;
use leptos::{prelude::*, reactive::computed::Memo};
use leptos_router::{
    NavigateOptions,
    hooks::{query_signal_with_options, use_location},
};
use wasm_bindgen::{JsCast, prelude::Closure};
use wasm_bindgen_futures::JsFuture;

use crate::BASE_URL;

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

/// Watches the router's URL hash and scrolls the element with that id into
/// view. Defers one animation frame so the navigated view has time to mount.
/// Call once near the top of a component that renders sections with `id=...`.
pub fn use_scroll_to_hash() {
    let location = use_location();
    Effect::new(move |_| {
        let hash = location.hash.get();
        let id = hash.trim_start_matches('#').to_string();
        if id.is_empty() {
            return;
        }
        let id = js_sys::decode_uri_component(&id)
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or(id);
        request_animation_frame(move || {
            if let Some(el) = document().get_element_by_id(&id) {
                el.scroll_into_view();
            }
        });
    });
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Main,
    Character,
    QuickStart,
    Reference,
    Share,
    Story,
}

impl PageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Character => "character",
            Self::QuickStart => "quick-start",
            Self::Reference => "reference",
            Self::Share => "share",
            Self::Story => "story",
        }
    }
}

/// Async sleep via JS `setTimeout` Promise.
pub async fn sleep(duration: Duration) {
    let ms = duration.as_millis().min(i32::MAX as u128) as i32;
    let promise = Promise::new(&mut move |resolve, _| {
        let _ = web_sys::window()
            .expect("no window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = JsFuture::from(promise).await;
}

pub fn use_page_kind() -> Memo<PageKind> {
    let pathname = use_location().pathname;
    Memo::new(move |_| {
        let path = pathname.read();
        let rest = path.strip_prefix(BASE_URL).unwrap_or(&path);
        let (first_seg, tail) = rest
            .strip_prefix('/')
            .unwrap_or(rest)
            .split_once('/')
            .unwrap_or((rest, ""));
        match first_seg {
            "c" if tail.ends_with("quick-start") => PageKind::QuickStart,
            "c" if tail.starts_with("story") => PageKind::Story,
            "c" => PageKind::Character,
            "r" => PageKind::Reference,
            "s" => PageKind::Share,
            _ => PageKind::Main,
        }
    })
}

/// Join an iterator of displayable items with a separator, without
/// allocating an intermediate `Vec`.
pub fn join_iter<I, D>(iter: I, sep: &str) -> String
where
    I: IntoIterator<Item = D>,
    D: std::fmt::Display,
{
    use std::fmt::Write;
    let mut result = String::new();
    for (i, item) in iter.into_iter().enumerate() {
        if i > 0 {
            result.push_str(sep);
        }
        let _ = write!(result, "{item}");
    }
    result
}
