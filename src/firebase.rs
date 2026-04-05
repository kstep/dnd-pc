use js_sys::{Array, Object, Promise, Reflect};
use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::JsFuture;

/// Timeout for Firestore operations in ms (setDoc can hang with offline
/// persistence).
const FIRESTORE_TIMEOUT_MS: u32 = 10_000;

#[derive(Debug)]
pub enum FirebaseError {
    NotAvailable,
    Timeout { method: &'static str },
    Js(JsValue),
    Serde(serde_wasm_bindgen::Error),
}

impl std::fmt::Display for FirebaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "Firebase not available"),
            Self::Timeout { method } => write!(f, "{method} timed out"),
            Self::Js(value) => write!(f, "{}", friendly_js_error(value)),
            Self::Serde(error) => write!(f, "{error}"),
        }
    }
}

impl From<JsValue> for FirebaseError {
    fn from(value: JsValue) -> Self {
        Self::Js(value)
    }
}

impl From<serde_wasm_bindgen::Error> for FirebaseError {
    fn from(error: serde_wasm_bindgen::Error) -> Self {
        Self::Serde(error)
    }
}

fn firebase_obj() -> Option<Object> {
    let window = web_sys::window()?;
    let val = Reflect::get(&window, &"__firebase".into()).ok()?;
    val.dyn_into::<Object>().ok()
}

pub fn is_available() -> bool {
    firebase_obj().is_some()
}

/// Wait for the Firebase JS module to initialize.
/// Returns `true` if Firebase became available, `false` if not configured.
pub async fn wait_ready() -> bool {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return false,
    };
    let promise = match Reflect::get(&window, &"__firebaseReady".into()) {
        Ok(val) => match val.dyn_into::<Promise>() {
            Ok(p) => p,
            Err(_) => return false,
        },
        Err(_) => return false,
    };
    let _ = JsFuture::from(promise).await;
    is_available()
}

/// Call a `window.__firebase` method and return the result as a Promise.
/// Fails immediately on structural errors (missing object, not a function).
fn call_to_promise(method: &str, args: &[JsValue]) -> Result<Promise, FirebaseError> {
    let fb = firebase_obj().ok_or(FirebaseError::NotAvailable)?;
    let func = Reflect::get(&fb, &method.into())?;
    let func: js_sys::Function = func
        .dyn_into()
        .map_err(|_| JsValue::from_str(&format!("__firebase.{method} is not a function")))?;
    let js_args: Array = args.iter().collect();
    let result = Reflect::apply(&func, &fb, &js_args)?;
    result
        .dyn_into()
        .map_err(|_| JsValue::from_str(&format!("__firebase.{method} did not return a Promise")))
        .map_err(FirebaseError::Js)
}

/// Call a `window.__firebase` method synchronously (non-Promise return value).
fn call(method: &str, args: &[JsValue]) -> Result<JsValue, FirebaseError> {
    let fb = firebase_obj().ok_or(FirebaseError::NotAvailable)?;
    let func = Reflect::get(&fb, &method.into())?;
    let func: js_sys::Function = func
        .dyn_into()
        .map_err(|_| JsValue::from_str(&format!("__firebase.{method} is not a function")))?;
    let js_args: Array = args.iter().collect();
    Ok(Reflect::apply(&func, &fb, &js_args)?)
}

/// Call a `window.__firebase` method and await its Promise (no retry, no
/// timeout). Used for auth operations that are interactive/non-retriable.
async fn call_async(method: &str, args: &[JsValue]) -> Result<JsValue, FirebaseError> {
    let promise = call_to_promise(method, args)?;
    Ok(JsFuture::from(promise).await?)
}

/// Create a JS Promise that resolves after `ms` milliseconds.
/// The resolved value is `sentinel` (use `JsValue::UNDEFINED` for sleep).
fn timeout_promise(ms: i32, sentinel: &JsValue) -> Promise {
    let sentinel = sentinel.clone();
    Promise::new(&mut move |resolve, _| {
        web_sys::window()
            .expect("no global window")
            .set_timeout_with_callback_and_timeout_and_arguments_1(&resolve, ms, &sentinel)
            .expect("setTimeout failed");
    })
}

/// Race a JS promise against a timeout. Returns `Err` if the timeout fires
/// first. Uses a sentinel object (not `undefined`) to distinguish timeout from
/// a successful resolve that returns `undefined` (e.g. Firestore `setDoc`).
async fn with_timeout(promise: Promise, method: &'static str) -> Result<JsValue, FirebaseError> {
    let sentinel = Object::new();
    let sentinel_val: JsValue = sentinel.clone().into();
    let timeout = timeout_promise(FIRESTORE_TIMEOUT_MS as i32, &sentinel_val);
    let race = Promise::race(&Array::of2(&promise, &timeout));
    let value = JsFuture::from(race).await?;
    if value == sentinel_val {
        Err(FirebaseError::Timeout { method })
    } else {
        Ok(value)
    }
}

/// Sleep for the given number of milliseconds.
async fn sleep_ms(ms: u32) {
    let promise = timeout_promise(ms as i32, &JsValue::UNDEFINED);
    let _ = JsFuture::from(promise).await;
}

/// Max retry attempts for Firestore operations.
const MAX_RETRIES: u32 = 3;
/// Initial retry delay in ms (doubles each attempt: 2s, 4s, 8s).
const INITIAL_RETRY_MS: u32 = 2_000;

/// Like `call_async` but with timeout and exponential backoff retry.
/// Structural errors (missing method, not a Promise) fail immediately.
/// Only timeouts and Promise rejections are retried.
async fn call_async_with_retry(
    method: &'static str,
    args: &[JsValue],
) -> Result<JsValue, FirebaseError> {
    let mut delay = INITIAL_RETRY_MS;
    let mut last_err = FirebaseError::Js(JsValue::UNDEFINED);

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            log::info!(
                "Retrying __firebase.{method} (attempt {attempt}/{MAX_RETRIES}) after {delay}ms"
            );
            sleep_ms(delay).await;
            delay *= 2;
        }

        // Fail fast on structural errors — these are permanent.
        let promise = call_to_promise(method, args)?;

        match with_timeout(promise, method).await {
            Ok(value) => return Ok(value),
            Err(error) => {
                last_err = error;
            }
        }
    }

    Err(last_err)
}

fn to_js<T: Serialize>(data: &T) -> Result<JsValue, FirebaseError> {
    let serializer = serde_wasm_bindgen::Serializer::new()
        .serialize_maps_as_objects(true)
        .serialize_missing_as_null(true);
    Ok(data.serialize(&serializer)?)
}

fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, FirebaseError> {
    Ok(serde_wasm_bindgen::from_value(value)?)
}

fn from_js_array<T: DeserializeOwned>(
    value: JsValue,
    label: &str,
) -> Result<Vec<T>, FirebaseError> {
    let array: Array = value.dyn_into().map_err(|_| {
        FirebaseError::Js(JsValue::from_str(&format!(
            "{label} did not return an array"
        )))
    })?;
    let mut items = Vec::with_capacity(array.length() as usize);
    for i in 0..array.length() {
        match from_js::<T>(array.get(i)) {
            Ok(val) => items.push(val),
            Err(error) => log::warn!("Failed to deserialize {label} at index {i}: {error}"),
        }
    }
    Ok(items)
}

pub async fn sign_in_anonymously() -> Result<JsValue, FirebaseError> {
    call_async("signInAnonymously", &[]).await
}

/// Call linkWithGoogle synchronously (opens popup in user gesture context),
/// returning a Promise to await for the result.
pub fn link_with_google_start() -> Result<Promise, FirebaseError> {
    let result = call("linkWithGoogle", &[])?;
    result.dyn_into::<Promise>().map_err(|_| {
        FirebaseError::Js(JsValue::from_str("linkWithGoogle did not return a Promise"))
    })
}

pub async fn link_with_google_finish(promise: Promise) -> Result<JsValue, FirebaseError> {
    Ok(JsFuture::from(promise).await?)
}

pub fn current_uid() -> Option<String> {
    call("currentUid", &[]).ok()?.as_string()
}

/// Wait for Firebase auth state to settle (including pending redirects).
/// Returns `(uid, is_anonymous)` or `None` if no user.
pub async fn wait_for_auth() -> Option<(String, bool)> {
    let result = call_async("waitForAuth", &[]).await.ok()?;
    if result.is_null() || result.is_undefined() {
        return None;
    }
    let uid = Reflect::get(&result, &"uid".into()).ok()?.as_string()?;
    let is_anon = Reflect::get(&result, &"isAnonymous".into())
        .ok()?
        .as_bool()
        .unwrap_or(true);
    Some((uid, is_anon))
}

// --- Generic Firestore operations ---

pub async fn set_doc(data: &impl Serialize, path: &[&str]) -> Result<(), FirebaseError> {
    let mut args = vec![to_js(data)?];
    args.extend(path.iter().map(|segment| JsValue::from_str(segment)));
    call_async_with_retry("setDoc", &args).await?;
    Ok(())
}

pub async fn get_doc<T: DeserializeOwned>(path: &[&str]) -> Result<Option<T>, FirebaseError> {
    let args: Vec<JsValue> = path
        .iter()
        .map(|segment| JsValue::from_str(segment))
        .collect();
    let result = call_async_with_retry("getDoc", &args).await?;
    if result.is_null() || result.is_undefined() {
        return Ok(None);
    }
    from_js(result).map(Some)
}

pub async fn get_all_docs<T: DeserializeOwned>(path: &[&str]) -> Result<Vec<T>, FirebaseError> {
    let args: Vec<JsValue> = path
        .iter()
        .map(|segment| JsValue::from_str(segment))
        .collect();
    let result = call_async_with_retry("getDocs", &args).await?;
    from_js_array(result, "getDocs")
}

pub async fn delete_doc(path: &[&str]) -> Result<(), FirebaseError> {
    let args: Vec<JsValue> = path
        .iter()
        .map(|segment| JsValue::from_str(segment))
        .collect();
    call_async_with_retry("deleteDoc", &args).await?;
    Ok(())
}

/// Extract a human-readable message from a JsValue error.
fn friendly_js_error(js_err: &JsValue) -> String {
    if let Some(error_obj) = js_err.dyn_ref::<js_sys::Error>() {
        return error_obj.message().into();
    }
    if let Some(text) = js_err.as_string() {
        return text;
    }
    format!("{js_err:?}")
}
