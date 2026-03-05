use js_sys::{Array, Object, Promise, Reflect};
use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// Timeout for Firestore operations in ms (setDoc can hang with offline persistence).
const FIRESTORE_TIMEOUT_MS: u32 = 10_000;

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

fn call(method: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let fb = firebase_obj().ok_or_else(|| JsValue::from_str("Firebase not available"))?;
    let func = Reflect::get(&fb, &method.into())?;
    let func: js_sys::Function = func
        .dyn_into()
        .map_err(|_| JsValue::from_str(&format!("__firebase.{method} is not a function")))?;
    let js_args: Array = args.iter().collect();
    Reflect::apply(&func, &fb, &js_args)
}

async fn call_async(method: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let result = call(method, args)?;
    let promise: Promise = result
        .dyn_into()
        .map_err(|_| JsValue::from_str(&format!("__firebase.{method} did not return a Promise")))?;
    JsFuture::from(promise).await
}

/// Like `call_async` but races against a timeout. Returns `Err` if the timeout
/// fires first (Firestore can hang indefinitely with offline persistence).
async fn call_async_with_timeout(method: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let result = call(method, args)?;
    let promise: Promise = result
        .dyn_into()
        .map_err(|_| JsValue::from_str(&format!("__firebase.{method} did not return a Promise")))?;

    let timeout = Promise::new(&mut |resolve, _| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, FIRESTORE_TIMEOUT_MS as i32)
            .unwrap();
    });
    let race = Promise::race(&Array::of2(&promise, &timeout));
    let value = JsFuture::from(race).await?;
    if value.is_undefined() {
        Err(JsValue::from_str(&format!(
            "__firebase.{method} timed out after {FIRESTORE_TIMEOUT_MS}ms"
        )))
    } else {
        Ok(value)
    }
}

pub async fn sign_in_anonymously() -> Result<JsValue, JsValue> {
    call_async("signInAnonymously", &[]).await
}

/// Call linkWithGoogle synchronously (opens popup in user gesture context),
/// returning a Promise to await for the result.
pub fn link_with_google_start() -> Result<Promise, JsValue> {
    let result = call("linkWithGoogle", &[])?;
    result
        .dyn_into::<Promise>()
        .map_err(|_| JsValue::from_str("linkWithGoogle did not return a Promise"))
}

pub async fn link_with_google_finish(promise: Promise) -> Result<JsValue, JsValue> {
    JsFuture::from(promise).await
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

pub async fn set_character_doc(uid: &str, char_id: &str, data: &Value) -> Result<(), JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::new()
        .serialize_maps_as_objects(true)
        .serialize_missing_as_null(true);
    let js_data = data
        .serialize(&serializer)
        .map_err(|error| JsValue::from_str(&format!("Serialization error: {error}")))?;
    call_async_with_timeout("setCharacterDoc", &[uid.into(), char_id.into(), js_data]).await?;
    Ok(())
}

pub async fn get_all_characters(uid: &str) -> Result<Vec<Value>, JsValue> {
    let result = call_async_with_timeout("getAllCharacters", &[uid.into()]).await?;
    let array: Array = result
        .dyn_into()
        .map_err(|_| JsValue::from_str("getAllCharacters did not return an array"))?;
    let mut chars = Vec::with_capacity(array.length() as usize);
    for i in 0..array.length() {
        let item = array.get(i);
        match serde_wasm_bindgen::from_value::<Value>(item) {
            Ok(val) => chars.push(val),
            Err(error) => {
                log::warn!("Failed to deserialize remote character at index {i}: {error}")
            }
        }
    }
    Ok(chars)
}

pub async fn delete_character_doc(uid: &str, char_id: &str) -> Result<(), JsValue> {
    call_async_with_timeout("deleteCharacterDoc", &[uid.into(), char_id.into()]).await?;
    Ok(())
}
