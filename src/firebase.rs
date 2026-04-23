use std::{borrow::Cow, cell::RefCell};

use js_sys::{Array, Object, Promise, Reflect};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use wasm_bindgen_futures::JsFuture;

/// Firebase Auth UID. Opaque ~28-char identifier from Firebase, not a UUID.
/// Newtype-wrapped so that future optimizations (inlining, Arc-sharing) are
/// localized.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FirebaseUid(String);

impl FirebaseUid {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for FirebaseUid {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl From<String> for FirebaseUid {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for FirebaseUid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Timeout for Firestore operations in ms (setDoc can hang with offline
/// persistence).
const FIRESTORE_TIMEOUT_MS: u32 = 10_000;

/// Describes why a bridge-contract check failed (JS API didn't return the
/// expected type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeReason {
    /// The JS property was not a callable function.
    NotAFunction,
    /// The function return value was not a `Promise`.
    NotAPromise,
    /// The return value was not a JS `Array`.
    NotAnArray,
    /// A stringify call returned a non-string value.
    NonStringResult,
}

#[derive(Debug)]
pub enum FirebaseError {
    NotAvailable,
    Timeout {
        method: &'static str,
    },
    /// A real JS error originating from a rejected Promise or JS runtime.
    Js(JsValue),
    Serde(serde_wasm_bindgen::Error),
    Json(serde_json::Error),
    /// The JS bridge violated the expected API contract.
    BridgeContract {
        method: Cow<'static, str>,
        reason: BridgeReason,
    },
    /// Multiple operations failed; carries the count and last real error.
    BatchFailed {
        count: u32,
        last: Box<FirebaseError>,
    },
}

impl std::fmt::Display for FirebaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAvailable => write!(f, "Firebase not available"),
            Self::Timeout { method } => write!(f, "{method} timed out"),
            Self::Js(value) => write!(f, "{}", friendly_js_error(value)),
            Self::Serde(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "JSON: {error}"),
            Self::BridgeContract { method, reason } => match reason {
                BridgeReason::NotAFunction => {
                    write!(f, "__firebase.{method} is not a function")
                }
                BridgeReason::NotAPromise => {
                    write!(f, "__firebase.{method} did not return a Promise")
                }
                BridgeReason::NotAnArray => {
                    write!(f, "{method} did not return an array")
                }
                BridgeReason::NonStringResult => {
                    write!(f, "{method} returned non-string")
                }
            },
            Self::BatchFailed { count, last } => {
                write!(f, "{count} operation(s) failed, last error: {last}")
            }
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

impl From<serde_json::Error> for FirebaseError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
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
fn call_to_promise(method: &'static str, args: &[JsValue]) -> Result<Promise, FirebaseError> {
    let fb = firebase_obj().ok_or(FirebaseError::NotAvailable)?;
    let func = Reflect::get(&fb, &method.into())?;
    let func: js_sys::Function = func.dyn_into().map_err(|_| FirebaseError::BridgeContract {
        method: Cow::Borrowed(method),
        reason: BridgeReason::NotAFunction,
    })?;
    let js_args: Array = args.iter().collect();
    let result = Reflect::apply(&func, &fb, &js_args)?;
    result
        .dyn_into()
        .map_err(|_| FirebaseError::BridgeContract {
            method: Cow::Borrowed(method),
            reason: BridgeReason::NotAPromise,
        })
}

/// Call a `window.__firebase` method synchronously (non-Promise return value).
fn call(method: &'static str, args: &[JsValue]) -> Result<JsValue, FirebaseError> {
    let fb = firebase_obj().ok_or(FirebaseError::NotAvailable)?;
    let func = Reflect::get(&fb, &method.into())?;
    let func: js_sys::Function = func.dyn_into().map_err(|_| FirebaseError::BridgeContract {
        method: Cow::Borrowed(method),
        reason: BridgeReason::NotAFunction,
    })?;
    let js_args: Array = args.iter().collect();
    Ok(Reflect::apply(&func, &fb, &js_args)?)
}

/// Call a `window.__firebase` method and await its Promise (no retry, no
/// timeout). Used for auth operations that are interactive/non-retriable.
async fn call_async(method: &'static str, args: &[JsValue]) -> Result<JsValue, FirebaseError> {
    let promise = call_to_promise(method, args)?;
    Ok(JsFuture::from(promise).await?)
}

/// Create a JS Promise that resolves after `ms` milliseconds.
/// The resolved value is `sentinel` (use `JsValue::UNDEFINED` for sleep).
fn timeout_promise(ms: u32, sentinel: &JsValue) -> Promise {
    let ms = ms.min(i32::MAX as u32) as i32;
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
    let timeout = timeout_promise(FIRESTORE_TIMEOUT_MS, &sentinel_val);
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
    let promise = timeout_promise(ms, &JsValue::UNDEFINED);
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
    // Round-trip through JSON because serde_wasm_bindgen rejects Maps with
    // non-string keys (Skill, DamageType, SpellSlotPool serialize as u8).
    // JSON.parse stringifies all keys, which Firestore accepts.
    let json = serde_json::to_string(data)?;
    js_sys::JSON::parse(&json).map_err(FirebaseError::Js)
}

fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, FirebaseError> {
    // Symmetric with `to_js`: round-trip through JSON so non-string map keys
    // (Skill, DamageType, SpellSlotPool — all `enum_serde_u8!`) deserialize
    // via `visit_str`, which serde_wasm_bindgen skips for `deserialize_u8`.
    let json = js_sys::JSON::stringify(&value)
        .map_err(FirebaseError::Js)?
        .as_string()
        .ok_or_else(|| FirebaseError::BridgeContract {
            method: Cow::Borrowed("JSON.stringify"),
            reason: BridgeReason::NonStringResult,
        })?;
    serde_json::from_str(&json).map_err(FirebaseError::Json)
}

fn from_js_array<T: DeserializeOwned>(
    value: JsValue,
    label: &'static str,
) -> Result<Vec<T>, FirebaseError> {
    let array: Array = value
        .dyn_into()
        .map_err(|_| FirebaseError::BridgeContract {
            method: Cow::Borrowed(label),
            reason: BridgeReason::NotAnArray,
        })?;
    let mut items = Vec::with_capacity(array.length() as usize);
    for (i, item) in array.iter().enumerate() {
        match from_js::<T>(item) {
            Ok(value) => items.push(value),
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
    result
        .dyn_into::<Promise>()
        .map_err(|_| FirebaseError::BridgeContract {
            method: Cow::Borrowed("linkWithGoogle"),
            reason: BridgeReason::NotAPromise,
        })
}

pub async fn link_with_google_finish(promise: Promise) -> Result<JsValue, FirebaseError> {
    Ok(JsFuture::from(promise).await?)
}

pub fn current_uid() -> Option<FirebaseUid> {
    call("currentUid", &[])
        .ok()?
        .as_string()
        .map(FirebaseUid::from)
}

/// Returns the sign-in provider for the current user: "google.com",
/// "anonymous", etc. `None` if no user is signed in.
pub fn current_provider() -> Option<String> {
    call("currentProvider", &[]).ok()?.as_string()
}

/// Fetch a Firebase ID token for the current user. Returns `None` if no user
/// is signed in. Firebase caches the token and refreshes ~5 minutes before
/// expiry, so this is cheap to call on every request.
pub async fn get_id_token() -> Result<Option<String>, FirebaseError> {
    let value = call_async("getIdToken", &[JsValue::from_bool(false)]).await?;
    Ok(value.as_string())
}

/// Wait for Firebase auth state to settle (including pending redirects).
/// Returns `(uid, is_anonymous)` or `None` if no user.
pub async fn wait_for_auth() -> Option<(FirebaseUid, bool)> {
    let result = call_async("waitForAuth", &[]).await.ok()?;
    if result.is_null() || result.is_undefined() {
        return None;
    }
    let uid = Reflect::get(&result, &"uid".into())
        .ok()?
        .as_string()
        .map(FirebaseUid::from)?;
    let is_anon = Reflect::get(&result, &"isAnonymous".into())
        .ok()?
        .as_bool()
        .unwrap_or(true);
    Some((uid, is_anon))
}

/// Cached `FieldValue.delete()` sentinel. The Firestore sentinel is a
/// singleton-by-type (SDK checks via `instanceof`, not identity), so one
/// fetched instance can be reused across every null leaf of every payload
/// for the lifetime of the tab. Only a successful bridge call is cached —
/// failures return `JsValue::NULL` and retry on the next call so a late-
/// ready bridge self-heals.
fn delete_field_sentinel() -> JsValue {
    thread_local! {
        static SENTINEL: RefCell<Option<JsValue>> = const { RefCell::new(None) };
    }
    SENTINEL.with(|cell| {
        if let Some(sentinel) = cell.borrow().as_ref() {
            return sentinel.clone();
        }
        match call("deleteField", &[]) {
            Ok(sentinel) => {
                *cell.borrow_mut() = Some(sentinel.clone());
                sentinel
            }
            Err(error) => {
                log::warn!(
                    "delete_field_sentinel: bridge call failed, falling back to null tombstones: {error:?}"
                );
                JsValue::NULL
            }
        }
    })
}

/// Walk a `JsValue` tree produced by `to_js` and replace every `null` leaf
/// on an object with `sentinel`. Does NOT descend into arrays:
/// `sparse_diff` treats arrays atomically (see `src/storage/diff.rs:36`),
/// so a `null` inside an array is a legitimate element, not a tombstone.
///
/// Used by `merge_doc` to convert sparse-diff tombstones into Firestore
/// `FieldValue.delete()` sentinels so the server actually removes the
/// field instead of storing a literal `null`.
fn strip_nulls_in_place(value: &JsValue, sentinel: &JsValue) {
    if !value.is_object() || Array::is_array(value) {
        return;
    }
    let obj: &Object = value.unchecked_ref();
    for key in Object::keys(obj) {
        let Ok(child) = Reflect::get(obj, &key) else {
            continue;
        };
        if child.is_null() {
            let _ = Reflect::set(obj, &key, sentinel);
        } else {
            strip_nulls_in_place(&child, sentinel);
        }
    }
}

// --- Generic Firestore operations ---

pub async fn set_doc(data: &impl Serialize, path: &[&str]) -> Result<(), FirebaseError> {
    let mut args = vec![to_js(data)?];
    args.extend(path.iter().map(|segment| JsValue::from_str(segment)));
    call_async_with_retry("setDoc", &args).await?;
    Ok(())
}

pub async fn merge_doc(data: &impl Serialize, path: &[&str]) -> Result<(), FirebaseError> {
    let payload = to_js(data)?;
    // Convert sparse_diff tombstones (literal nulls for removed map keys)
    // into Firestore FieldValue.delete() sentinels so the server removes the
    // field instead of storing a literal null. See
    // docs/superpowers/plans/2026-04-23-deletefield-tombstone-cleanup.md.
    strip_nulls_in_place(&payload, &delete_field_sentinel());
    let mut args = vec![payload];
    args.extend(path.iter().map(|segment| JsValue::from_str(segment)));
    call_async_with_retry("mergeDoc", &args).await?;
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

pub async fn get_all_docs<T: DeserializeOwned>(
    path: &[&str],
    conditions: &[Where],
) -> Result<Vec<T>, FirebaseError> {
    let js_conditions = Where::as_js(conditions);
    let mut args: Vec<JsValue> = vec![js_conditions];
    args.extend(path.iter().map(|segment| JsValue::from_str(segment)));
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

// --- Realtime subscriptions ---

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
}

#[derive(Deserialize)]
pub struct DocChange {
    #[serde(rename = "type")]
    pub change_type: ChangeType,
    pub data: serde_json::Value,
    pub id: String,
}

pub struct Where(pub &'static str, pub &'static str, pub JsValue);

impl Where {
    pub fn gt(field: &'static str, value: impl Into<JsValue>) -> Self {
        Self(field, ">", value.into())
    }

    pub fn as_js(conditions: &[Where]) -> JsValue {
        if conditions.is_empty() {
            return JsValue::NULL;
        }
        let arr = Array::new();
        for clause in conditions {
            let triple = Array::of3(
                &JsValue::from_str(clause.0),
                &JsValue::from_str(clause.1),
                &clause.2,
            );
            arr.push(&triple);
        }
        arr.into()
    }
}

/// An active Firestore subscription. Dropping it unsubscribes.
pub struct Subscription {
    _callback: Closure<dyn Fn(JsValue)>,
    unsubscribe: js_sys::Function,
}

impl Subscription {
    pub fn unsubscribe(&self) {
        let _ = self.unsubscribe.call0(&JsValue::NULL);
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.unsubscribe();
    }
}

/// Subscribe to Firestore collection changes with optional query conditions.
/// The returned `Subscription` must be kept alive; dropping it unsubscribes.
pub fn subscribe_collection(
    path: &[&str],
    conditions: &[Where],
    on_change: impl Fn(Vec<DocChange>) + 'static,
) -> Result<Subscription, FirebaseError> {
    let callback =
        Closure::wrap(Box::new(
            move |changes: JsValue| match from_js::<Vec<DocChange>>(changes) {
                Ok(parsed) => on_change(parsed),
                Err(error) => log::warn!("Failed to parse Firestore snapshot batch: {error}"),
            },
        ) as Box<dyn Fn(JsValue)>);

    let js_conditions = Where::as_js(conditions);

    let mut args: Vec<JsValue> = vec![callback.as_ref().clone(), js_conditions];
    args.extend(path.iter().map(|segment| JsValue::from_str(segment)));
    let unsubscribe: js_sys::Function = call("onSnapshot", &args)?
        .dyn_into()
        .map_err(FirebaseError::from)?;

    Ok(Subscription {
        _callback: callback,
        unsubscribe,
    })
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

#[cfg(test)]
mod tests {
    use js_sys::{Array, Reflect};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_test::*;

    use super::{from_js, strip_nulls_in_place, to_js};
    use crate::{
        constvec::ConstVec,
        model::{
            Character, DamageModifier, DamageType, ProficiencyLevel, Skill, SpellSlotLevel,
            SpellSlotPool,
        },
    };

    wasm_bindgen_test_configure!(run_in_browser);

    /// Build a JS object from a JSON literal via js_sys::JSON::parse —
    /// cleaner than manually constructing Object/Array trees.
    fn js(json: &str) -> JsValue {
        js_sys::JSON::parse(json).expect("valid JSON literal")
    }

    #[wasm_bindgen_test]
    fn replaces_null_leaves_with_sentinel() {
        let value = js(r#"{"keep": 1, "drop": null}"#);
        let sentinel = JsValue::from_str("__SENTINEL__");
        strip_nulls_in_place(&value, &sentinel);

        let keep = Reflect::get(&value, &"keep".into()).unwrap();
        let drop = Reflect::get(&value, &"drop".into()).unwrap();
        assert_eq!(keep.as_f64(), Some(1.0));
        assert_eq!(drop.as_string(), Some("__SENTINEL__".into()));
    }

    #[wasm_bindgen_test]
    fn recurses_into_nested_objects() {
        let value = js(r#"{"outer": {"inner": null, "kept": 2}}"#);
        let sentinel = JsValue::from_str("__S__");
        strip_nulls_in_place(&value, &sentinel);

        let outer = Reflect::get(&value, &"outer".into()).unwrap();
        let inner = Reflect::get(&outer, &"inner".into()).unwrap();
        let kept = Reflect::get(&outer, &"kept".into()).unwrap();
        assert_eq!(inner.as_string(), Some("__S__".into()));
        assert_eq!(kept.as_f64(), Some(2.0));
    }

    #[wasm_bindgen_test]
    fn arrays_are_atomic_contents_untouched() {
        // sparse_diff treats arrays atomically (src/storage/diff.rs:36), so
        // null INSIDE an array is a legitimate array element, not a tombstone.
        // strip_nulls_in_place must not descend into arrays.
        let value = js(r#"{"list": [1, null, 3]}"#);
        let sentinel = JsValue::from_str("__S__");
        strip_nulls_in_place(&value, &sentinel);

        let list = Reflect::get(&value, &"list".into()).unwrap();
        let arr: Array = list.dyn_into().expect("is array");
        assert_eq!(arr.get(0).as_f64(), Some(1.0));
        assert!(arr.get(1).is_null(), "null array element must survive");
        assert_eq!(arr.get(2).as_f64(), Some(3.0));
    }

    #[wasm_bindgen_test]
    fn null_sentinel_leaves_null_leaves_untouched() {
        // Documents the graceful-degradation contract: if the bridge call
        // in `delete_field_sentinel` fails, it returns `JsValue::NULL`, and
        // the resulting payload must still be well-formed (null in, null
        // out). The read-side `deserialize_map_dropping_nulls` handles the
        // fallback.
        let value = js(r#"{"keep": 1, "drop": null}"#);
        strip_nulls_in_place(&value, &JsValue::NULL);

        let keep = Reflect::get(&value, &"keep".into()).unwrap();
        let drop = Reflect::get(&value, &"drop".into()).unwrap();
        assert_eq!(keep.as_f64(), Some(1.0));
        assert!(drop.is_null());
    }

    /// Round-trip a Character with maps keyed by u8-serialized enums
    /// (Skill, DamageType, SpellSlotPool). Old `to_js` blew up here; the
    /// JSON round-trip stringifies the keys so Firestore accepts them, and
    /// `enum_serde_u8!` parses both number and string variants on the way
    /// back.
    #[wasm_bindgen_test]
    fn to_js_handles_u8_keyed_maps() {
        let mut character = Character::default();
        character
            .skills
            .set(Skill::Athletics, ProficiencyLevel::Proficient);
        character
            .skills
            .set(Skill::Stealth, ProficiencyLevel::Expertise);
        character.damage_modifiers.insert(
            DamageType::Fire,
            DamageModifier {
                resistant: true,
                ..Default::default()
            },
        );
        let mut slots: ConstVec<SpellSlotLevel, 9> = ConstVec::default();
        slots[0] = SpellSlotLevel { total: 4, used: 1 };
        character.spell_slots.insert(SpellSlotPool::Arcane, slots);

        let js = to_js(&character).expect("to_js must not fail on u8-keyed maps");
        let restored: Character = from_js(js).expect("round-trip must deserialize");

        assert_eq!(
            restored.skills.get(Skill::Athletics),
            ProficiencyLevel::Proficient,
        );
        assert_eq!(
            restored.skills.get(Skill::Stealth),
            ProficiencyLevel::Expertise,
        );
        assert!(
            restored
                .damage_modifiers
                .get(&DamageType::Fire)
                .map(|m| m.resistant)
                .unwrap_or(false)
        );
        let arcane = restored
            .spell_slots
            .get(&SpellSlotPool::Arcane)
            .expect("arcane slots present");
        assert_eq!(arcane[0].total, 4);
        assert_eq!(arcane[0].used, 1);
    }
}
