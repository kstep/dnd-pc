use std::time::Duration;

use js_sys::{Array, Reflect};
use leptos::{leptos_dom::helpers::set_timeout, prelude::*};
use leptos_fluent::tr;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Blob, BlobPropertyBag, File, FilePropertyBag, HtmlAnchorElement, ShareData, Url};

use crate::{components::toast::Toast, model::Character};

pub fn export_character(character: &Character) {
    let json = match serde_json::to_string_pretty(character) {
        Ok(json) => json,
        Err(error) => {
            log::error!("Failed to serialize character: {error}");
            return;
        }
    };

    let filename = if character.identity.name.is_empty() {
        "character.dnd.json".to_string()
    } else {
        format!("{}.dnd.json", character.identity.name)
    };

    // Telegram Mini App has no working download or Web Share path (Android
    // WebView doesn't forward user activation to share intents and has no
    // download manager). Fall back to the clipboard + an explanatory toast
    // so the user can paste the JSON into Saved Messages.
    if is_telegram_mini_app() {
        export_to_clipboard(&json);
        return;
    }

    if try_web_share(&json, &filename) {
        return;
    }

    download_via_anchor(&json, &filename);
}

pub(crate) fn is_telegram_mini_app() -> bool {
    // Detection is heuristic — Telegram doesn't officially document how to
    // recognise a Mini App context. We match the set of signals used by
    // @telegram-apps/sdk:
    //  1. Native bridge objects `TelegramWebviewProxy` /
    //     `TelegramWebviewProxyProto` injected by the Android/iOS clients.
    //  2. Hash params like `#tgWebAppPlatform=…` added by Telegram's web clients
    //     (webk/weba) when launching a Mini App.
    let window = window();
    let win: &JsValue = window.as_ref();
    if has_property(win, "TelegramWebviewProxy") || has_property(win, "TelegramWebviewProxyProto") {
        return true;
    }
    let hash = window.location().hash().unwrap_or_default();
    let fragment = hash.strip_prefix('#').unwrap_or(&hash);
    fragment
        .split('&')
        .any(|pair| pair.starts_with("tgWebAppPlatform="))
}

fn has_property(target: &JsValue, name: &str) -> bool {
    Reflect::has(target, &JsValue::from_str(name)).unwrap_or(false)
}

pub(crate) fn copy_to_clipboard(text: &str) {
    if !window().is_secure_context() {
        return;
    }
    let promise = window().navigator().clipboard().write_text(text);
    spawn_local(async move {
        let _ = JsFuture::from(promise).await;
    });
}

fn export_to_clipboard(json: &str) {
    let promise = window().navigator().clipboard().write_text(json);
    // Build both toasts up-front in the current owner context. Each toast
    // captures the owner internally, so `.show()` is safe to call from the
    // async block where no owner is active.
    let ok_toast = Toast::new(tr!("toast-export-copied"));
    let fail_toast = Toast::new(tr!("toast-export-copy-failed"));
    spawn_local(async move {
        if JsFuture::from(promise).await.is_ok() {
            ok_toast.show();
        } else {
            fail_toast.show();
        }
    });
}

/// Try the Web Share API (level 2). Must be called synchronously from a
/// user gesture so the browser captures activation. Chrome Android's
/// shareable-file allowlist rejects `.json` files regardless of MIME, so
/// we share as `<filename>.txt` with `text/plain` MIME — the receiving
/// app still gets the JSON payload. If `share()` rejects for any reason
/// other than user-cancel, we fall back to `<a download>`.
fn try_web_share(json: &str, base_filename: &str) -> bool {
    let navigator = window().navigator();
    let navigator_js: &JsValue = navigator.as_ref();
    let has_method =
        |name: &str| Reflect::has(navigator_js, &JsValue::from_str(name)).unwrap_or(false);
    if !has_method("share") || !has_method("canShare") {
        return false;
    }

    let share_filename = format!("{base_filename}.txt");
    let parts = Array::new();
    parts.push(&JsValue::from_str(json));
    let file_opts = FilePropertyBag::new();
    file_opts.set_type("text/plain");
    let file = match File::new_with_blob_sequence_and_options(&parts, &share_filename, &file_opts) {
        Ok(file) => file,
        Err(error) => {
            log::warn!("Failed to build File for share: {error:?}");
            return false;
        }
    };
    let files = Array::new();
    files.push(&file);

    let data = ShareData::new();
    data.set_files(&files);
    data.set_title(base_filename);

    if !navigator.can_share_with_data(&data) {
        return false;
    }

    let promise = navigator.share_with_data(&data);
    let json_for_fallback = json.to_string();
    let filename_for_fallback = base_filename.to_string();
    let owner = Owner::current();
    spawn_local(async move {
        let Err(error) = JsFuture::from(promise).await else {
            return;
        };
        let name = Reflect::get(&error, &JsValue::from_str("name"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        if name == "AbortError" {
            return;
        }
        log::warn!("navigator.share failed ({name}), falling back to download: {error:?}");
        let run = move || download_via_anchor(&json_for_fallback, &filename_for_fallback);
        match owner {
            Some(owner) => owner.with(run),
            None => run(),
        }
    });
    true
}

fn download_via_anchor(json: &str, filename: &str) {
    let array = Array::new();
    array.push(&JsValue::from_str(json));
    let blob_opts = BlobPropertyBag::new();
    blob_opts.set_type("application/json");
    let blob = match Blob::new_with_str_sequence_and_options(&array, &blob_opts) {
        Ok(blob) => blob,
        Err(error) => {
            log::error!("Failed to create blob: {error:?}");
            return;
        }
    };

    let url = match Url::create_object_url_with_blob(&blob) {
        Ok(url) => url,
        Err(error) => {
            log::error!("Failed to create object URL: {error:?}");
            return;
        }
    };

    let document = document();
    let anchor: HtmlAnchorElement = document.create_element("a").unwrap().unchecked_into();
    anchor.set_href(&url);
    anchor.set_download(filename);
    let body = document.body().expect("document has body");
    let _ = body.append_child(&anchor);
    anchor.click();

    // Defer cleanup so the browser has time to start the download. Removing
    // the anchor synchronously or revoking the blob URL early can abort the
    // fetch in Chrome and mobile WebViews.
    let anchor_to_remove = anchor.clone();
    let body_for_cleanup = body.clone();
    let url_to_revoke = url.clone();
    set_timeout(
        move || {
            let _ = body_for_cleanup.remove_child(&anchor_to_remove);
            let _ = Url::revoke_object_url(&url_to_revoke);
        },
        Duration::from_secs(30),
    );
}
