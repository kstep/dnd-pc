use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{
    firebase,
    model::{Character, CharacterIndex, CharacterSummary, DamageType},
};

const INDEX_KEY: &str = "dnd_pc_index";
/// 2 s debounce — balances responsiveness vs. Firestore write-per-second cost.
const DEBOUNCE_MS: i32 = 2000;

fn character_key(id: &Uuid) -> String {
    format!("dnd_pc_char_{id}")
}

pub fn load_index() -> CharacterIndex {
    INDEX_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        cache
            .get_or_insert_with(|| LocalStorage::get(INDEX_KEY).unwrap_or_default())
            .clone()
    })
}

pub fn save_index(index: &CharacterIndex) {
    LocalStorage::set(INDEX_KEY, index).expect("failed to save index");
    INDEX_CACHE.with(|cell| {
        *cell.borrow_mut() = Some(index.clone());
    });
}

/// Update the index in place without a full load/save round-trip.
fn update_index(f: impl FnOnce(&mut CharacterIndex)) {
    INDEX_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        let index = cache.get_or_insert_with(|| LocalStorage::get(INDEX_KEY).unwrap_or_default());
        f(index);
        LocalStorage::set(INDEX_KEY, &*index).expect("failed to save index");
    });
}

/// Migrate legacy string damage_type values to u8 enum representation.
fn migrate_v1(value: &mut serde_json::Value) {
    if let Some(weapons) = value
        .get_mut("equipment")
        .and_then(|e| e.get_mut("weapons"))
        .and_then(|w| w.as_array_mut())
    {
        for weapon in weapons {
            if let Some(dt) = weapon.get("damage_type").and_then(|v| v.as_str()) {
                let new_val = match DamageType::from_name(dt) {
                    Some(d) => serde_json::Value::Number((d as u8).into()),
                    None => serde_json::Value::Null,
                };
                weapon["damage_type"] = new_val;
            }
        }
    }
}

/// Migrate flat spell_slots array to BTreeMap keyed by pool.
fn migrate_v2(value: &mut serde_json::Value) {
    if value
        .get("spell_slots")
        .is_some_and(|slots| slots.is_array())
    {
        let slots = value["spell_slots"].take();
        value["spell_slots"] = serde_json::json!({ "0": slots });
    }
}

/// Migrate string attack_bonus to i32.
fn migrate_v3(value: &mut serde_json::Value) {
    if let Some(weapons) = value
        .get_mut("equipment")
        .and_then(|e| e.get_mut("weapons"))
        .and_then(|w| w.as_array_mut())
    {
        for weapon in weapons {
            if let Some(s) = weapon.get("attack_bonus").and_then(|v| v.as_str()) {
                let parsed: i32 = s.parse().unwrap_or(0);
                weapon["attack_bonus"] = serde_json::Value::Number(parsed.into());
            }
        }
    }
}

pub fn load_character(id: &Uuid) -> Option<Character> {
    let key = character_key(id);
    if let Ok(ch) = LocalStorage::get::<Character>(&key) {
        return Some(ch);
    }
    // Fallback: migrate legacy format
    let raw = LocalStorage::raw().get_item(&key).ok()??;
    let mut value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    migrate_v1(&mut value);
    migrate_v2(&mut value);
    migrate_v3(&mut value);
    serde_json::from_value(value).ok()
}

fn upsert_index_entry(index: &mut CharacterIndex, summary: CharacterSummary) {
    if let Some(entry) = index.characters.iter_mut().find(|c| c.id == summary.id) {
        *entry = summary;
    } else {
        index.characters.push(summary);
    }
}

pub fn save_character(character: &mut Character) {
    // Save already in flight (cloud-pulled data written to Store) — skip to
    // avoid bumping updated_at and re-pushing to cloud.
    if SAVE_IN_FLIGHT.with(|flag| flag.replace(false)) {
        return;
    }

    character.touch();
    LocalStorage::set(character_key(&character.id), &*character).expect("failed to save character");

    let summary = character.summary();
    update_index(|index| upsert_index_entry(index, summary));

    // Debounced cloud push
    schedule_cloud_push(character);
}

pub fn delete_character(id: &Uuid) {
    LocalStorage::delete(character_key(id));

    let id = *id;
    update_index(|index| index.characters.retain(|c| c.id != id));

    let Some(uid) = firebase::current_uid() else {
        return;
    };

    // Cloud delete
    spawn_local(async move {
        if let Err(error) = delete_from_cloud(&uid, &id).await {
            log::warn!("Cloud delete failed: {error:?}");
        }
    });
}

/// Open a `.json` file picker, read the selected file, and call `on_character`
/// with the parsed [`Character`]. Shows a browser alert and logs on error.
pub fn pick_character_from_file<F: Fn(Character) + 'static>(on_character: F) {
    let on_character = Rc::new(on_character);
    let input: web_sys::HtmlInputElement =
        document().create_element("input").unwrap().unchecked_into();

    input.set_type("file");
    input.set_accept(".json");

    let input_clone = input.clone();
    let onchange_js = Closure::once_into_js(move || {
        let Some(files) = input_clone.files() else {
            return;
        };
        let Some(file) = files.get(0) else {
            return;
        };

        let reader = match web_sys::FileReader::new() {
            Ok(r) => r,
            Err(error) => {
                log::error!("Failed to create FileReader: {error:?}");
                return;
            }
        };

        let reader_clone = reader.clone();
        let onload_js = Closure::once_into_js(move || {
            let result = match reader_clone.result() {
                Ok(r) => r,
                Err(error) => {
                    log::error!("Failed to read file: {error:?}");
                    return;
                }
            };
            let Some(text) = result.as_string() else {
                log::error!("File result is not a string");
                return;
            };
            match serde_json::from_str::<Character>(&text) {
                Ok(character) => on_character(character),
                Err(error) => {
                    log::error!("Failed to parse character JSON: {error}");
                    window()
                        .alert_with_message(&format!("Invalid character file: {error}"))
                        .ok();
                }
            }
        });

        reader.set_onload(Some(onload_js.unchecked_ref()));

        if let Err(error) = reader.read_as_text(&file) {
            log::error!("Failed to start reading file: {error:?}");
        }
    });

    input.set_onchange(Some(onchange_js.unchecked_ref()));

    input.click();
}

// --------------- Cloud Sync ---------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    Disabled,
    Connecting,
    Synced,
    Syncing,
    Error,
}

#[derive(Clone, Copy)]
struct SyncState {
    status: RwSignal<SyncStatus>,
    uid: RwSignal<Option<String>>,
    anon: RwSignal<bool>,
    last_error: RwSignal<Option<String>>,
    /// Bumped after cloud pull modifies the character index, so the UI can
    /// react.
    index_version: RwSignal<u32>,
}

impl SyncState {
    fn set_error(&self, msg: String) {
        self.last_error.set(Some(msg));
        self.status.set(SyncStatus::Error);
    }

    fn set_ok(&self, status: SyncStatus) {
        self.last_error.set(None);
        self.status.set(status);
    }
}

thread_local! {
    static SYNC_STATE: RefCell<Option<SyncState>> = const { RefCell::new(None) };
    /// Maps character UUID to (timer_id, closure JsValue). Storing the JsValue
    /// prevents the closure from leaking — it is dropped when the timer is
    /// cancelled or after it fires.
    static DEBOUNCE_TIMERS: RefCell<HashMap<Uuid, (i32, JsValue)>> = RefCell::new(HashMap::new());
    /// Cached character index to avoid repeated localStorage round-trips on every
    /// save. Lazily populated on first access; kept in sync with localStorage.
    static INDEX_CACHE: RefCell<Option<CharacterIndex>> = const { RefCell::new(None) };
    /// Set when cloud-pulled data is being written to the Store. The auto-save
    /// effect will fire but `save_character` skips because the save is already
    /// handled by the sync pipeline.
    static SAVE_IN_FLIGHT: Cell<bool> = const { Cell::new(false) };
}

fn get_or_init_sync() -> SyncState {
    SYNC_STATE.with(|cell| {
        let mut opt = cell.borrow_mut();
        *opt.get_or_insert_with(|| SyncState {
            status: RwSignal::new(SyncStatus::Disabled),
            uid: RwSignal::new(None),
            anon: RwSignal::new(false),
            last_error: RwSignal::new(None),
            index_version: RwSignal::new(0),
        })
    })
}

pub fn sync_status() -> ReadSignal<SyncStatus> {
    get_or_init_sync().status.read_only()
}

#[allow(dead_code)]
pub fn sync_uid() -> ReadSignal<Option<String>> {
    get_or_init_sync().uid.read_only()
}

pub fn sync_is_anonymous() -> ReadSignal<bool> {
    get_or_init_sync().anon.read_only()
}

pub fn sync_last_error() -> ReadSignal<Option<String>> {
    get_or_init_sync().last_error.read_only()
}

/// Reactive signal bumped whenever cloud pull updates the character index.
/// Components can track this to refresh their view of the index.
pub fn sync_index_version() -> ReadSignal<u32> {
    get_or_init_sync().index_version.read_only()
}

/// Set up a reactive effect that reloads a character from localStorage when
/// cloud sync pulls updates. Calls `on_update` with the fresh data when the
/// remote version is newer than `current_updated_at()`.
///
/// Sets `SAVE_IN_FLIGHT` before calling `on_update` so the auto-save
/// effect (which fires when the Store is updated) skips the redundant
/// save and cloud re-push.
pub fn track_cloud_character(
    id: Uuid,
    current_updated_at: impl Fn() -> u64 + 'static,
    on_update: impl Fn(Character) + 'static,
) {
    let index_version = sync_index_version();
    Effect::new(move |previous: Option<u32>| {
        if previous.is_some()
            && let Some(character) = load_character(&id)
            && character.updated_at > current_updated_at()
        {
            SAVE_IN_FLIGHT.with(|flag| flag.set(true));
            on_update(character);
        }
        index_version.get()
    });
}

/// Shared post-sign-in logic: resolve UID, run cloud sync operations, update
/// status. Returns `Err` only if UID is missing after auth.
async fn finish_sign_in(state: SyncState, is_anon: bool, sync_op: SyncOp) {
    state.anon.set(is_anon);
    match firebase::current_uid() {
        Some(uid) => {
            log::info!("finish_sign_in: uid={uid}, op={sync_op:?}");
            state.uid.set(Some(uid));
            state.set_ok(SyncStatus::Syncing);
            let push_local_only = matches!(sync_op, SyncOp::FullSync);
            let last_err = match sync_all_with_cloud(push_local_only).await {
                Ok(()) => None,
                Err(error) => {
                    log::warn!("Cloud sync failed: {error:?}");
                    Some(format!(
                        "Sync failed: {}",
                        firebase::friendly_js_error(&error)
                    ))
                }
            };
            log::info!("finish_sign_in: done");
            match last_err {
                Some(msg) => state.set_error(msg),
                None => state.set_ok(SyncStatus::Synced),
            }
        }
        None => {
            state.uid.set(None);
            state.set_error("No UID after sign-in".into());
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SyncOp {
    /// Pull remote characters, push local-newer ones (anonymous users).
    PullOnly,
    /// Full bidirectional sync: pull remote, push local-newer and local-only
    /// characters (authenticated users).
    FullSync,
}

impl SyncOp {
    fn for_anon(is_anon: bool) -> Self {
        if is_anon {
            Self::PullOnly
        } else {
            Self::FullSync
        }
    }
}

pub fn init_sync() {
    let state = get_or_init_sync();
    state.set_ok(SyncStatus::Connecting);

    spawn_local(async move {
        if !firebase::wait_ready().await {
            log::info!("Firebase not available, cloud sync disabled");
            state.status.set(SyncStatus::Disabled);
            return;
        }

        // Wait for auth state to settle (handles redirects, restored sessions, etc.)
        if let Some((uid, is_anon)) = firebase::wait_for_auth().await {
            log::info!("Auth settled: uid={uid}, anon={is_anon}");
            finish_sign_in(state, is_anon, SyncOp::for_anon(is_anon)).await;
            return;
        }

        // No existing session — do anonymous sign-in
        log::info!("No existing session, signing in anonymously");
        match firebase::sign_in_anonymously().await {
            Ok(_) => finish_sign_in(state, true, SyncOp::PullOnly).await,
            Err(error) => {
                log::warn!("Anonymous sign-in failed: {error:?}");
                state.set_error(format!(
                    "Anonymous sign-in failed: {}",
                    firebase::friendly_js_error(&error)
                ));
            }
        }
    });
}

pub fn sign_in_with_google() {
    if !firebase::is_available() {
        return;
    }
    let state = get_or_init_sync();
    state.set_ok(SyncStatus::Connecting);

    // Open popup synchronously to preserve user gesture context.
    // Browsers block popups that aren't triggered by a direct user click.
    let promise = match firebase::link_with_google_start() {
        Ok(p) => p,
        Err(error) => {
            log::warn!("Google sign-in failed: {error:?}");
            state.set_error(format!(
                "Google sign-in failed: {}",
                firebase::friendly_js_error(&error)
            ));
            return;
        }
    };

    spawn_local(async move {
        match firebase::link_with_google_finish(promise).await {
            Ok(_) => finish_sign_in(state, false, SyncOp::FullSync).await,
            Err(error) => {
                log::warn!("Google sign-in failed: {error:?}");
                state.set_error(format!(
                    "Google sign-in failed: {}",
                    firebase::friendly_js_error(&error)
                ));
            }
        }
    });
}

pub fn retry_sync() {
    let state = get_or_init_sync();
    if state.uid.get_untracked().is_none() {
        // No UID — re-run full init
        init_sync();
        return;
    }
    let is_anon = state.anon.get_untracked();
    state.set_ok(SyncStatus::Syncing);
    spawn_local(async move {
        finish_sign_in(state, is_anon, SyncOp::for_anon(is_anon)).await;
    });
}

fn schedule_cloud_push(character: &Character) {
    if !firebase::is_available() {
        return;
    }
    if get_or_init_sync().uid.get_untracked().is_none() {
        return;
    }

    let char_id = character.id;

    // Cancel existing debounce timer for this character (drops old closure JsValue)
    DEBOUNCE_TIMERS.with(|timers| {
        if let Some((old_timer, _)) = timers.borrow_mut().remove(&char_id) {
            window().clear_timeout_with_handle(old_timer);
        }
    });

    // Defer serialization: read raw JSON from localStorage when the timer fires,
    // parse directly to serde_json::Value, skipping Character deserialization.
    let closure_js = Closure::once_into_js(move || {
        DEBOUNCE_TIMERS.with(|timers| {
            timers.borrow_mut().remove(&char_id);
        });
        let state = get_or_init_sync();
        spawn_local(async move {
            let Some(uid) = firebase::current_uid() else {
                return;
            };
            let key = character_key(&char_id);
            let Ok(Some(raw)) = LocalStorage::raw().get_item(&key) else {
                return;
            };
            let json: serde_json::Value = match serde_json::from_str(&raw) {
                Ok(value) => value,
                Err(error) => {
                    log::warn!("Failed to parse character JSON for cloud: {error}");
                    return;
                }
            };
            state.set_ok(SyncStatus::Syncing);
            let char_id_str = char_id.to_string();
            match firebase::set_character_doc(&uid, &char_id_str, &json).await {
                Ok(()) => state.set_ok(SyncStatus::Synced),
                Err(error) => {
                    log::warn!("Cloud push failed: {error:?}");
                    state.set_error(format!(
                        "Push failed: {}",
                        firebase::friendly_js_error(&error)
                    ));
                }
            }
        });
    });

    match window().set_timeout_with_callback_and_timeout_and_arguments_0(
        closure_js.unchecked_ref(),
        DEBOUNCE_MS,
    ) {
        Ok(timer_id) => {
            DEBOUNCE_TIMERS.with(|timers| {
                timers.borrow_mut().insert(char_id, (timer_id, closure_js));
            });
        }
        Err(error) => log::warn!("set_timeout failed: {error:?}"),
    }
}

async fn push_to_cloud(uid: &str, character: &Character) -> Result<(), JsValue> {
    let json = serde_json::to_value(character)
        .map_err(|error| JsValue::from_str(&format!("Serialization error: {error}")))?;
    firebase::set_character_doc(uid, &character.id.to_string(), &json).await
}

async fn delete_from_cloud(uid: &str, id: &Uuid) -> Result<(), JsValue> {
    firebase::delete_character_doc(uid, &id.to_string()).await
}

/// Bidirectional sync: pull remote characters (saving remote-newer locally,
/// pushing local-newer to cloud). When `push_local_only` is true, also push
/// characters that exist only locally (e.g. after linking a Google account).
async fn sync_all_with_cloud(push_local_only: bool) -> Result<(), JsValue> {
    let Some(uid) = firebase::current_uid() else {
        log::info!("sync_all_with_cloud: no UID, skipping");
        return Ok(());
    };
    log::info!("sync_all_with_cloud: syncing for uid={uid}");
    let remote_chars = firebase::get_all_characters(&uid).await?;
    log::info!(
        "sync_all_with_cloud: got {} remote characters",
        remote_chars.len()
    );

    let mut index = load_index();
    let mut pos_map: HashMap<Uuid, usize> = index
        .characters
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id, i))
        .collect();
    let mut index_dirty = false;
    let mut push_failures: u32 = 0;
    let mut seen_remote: HashSet<Uuid> = HashSet::with_capacity(remote_chars.len());

    for remote_value in remote_chars {
        let remote: Character = match serde_json::from_value(remote_value) {
            Ok(character) => character,
            Err(error) => {
                log::warn!("Failed to deserialize remote character: {error}");
                continue;
            }
        };
        seen_remote.insert(remote.id);

        // Check local timestamp from the index (already in memory) to avoid
        // unnecessary full Character deserialization from localStorage.
        let local_updated_at = pos_map
            .get(&remote.id)
            .map(|&position| index.characters[position].updated_at)
            .unwrap_or(0);

        if local_updated_at >= remote.updated_at {
            // Local is same or newer — push local to cloud if strictly newer
            if local_updated_at > remote.updated_at
                && let Some(local_character) = load_character(&remote.id)
                && let Err(error) = push_to_cloud(&uid, &local_character).await
            {
                log::warn!("Failed to push local-newer character: {error:?}");
                push_failures += 1;
            }
        } else {
            // Remote is newer or doesn't exist locally — save to localStorage
            if let Err(error) = LocalStorage::set(character_key(&remote.id), &remote) {
                log::warn!("Failed to save pulled character {}: {error}", remote.id);
                continue;
            }

            let summary = remote.summary();
            match pos_map.get(&remote.id) {
                Some(&position) => index.characters[position] = summary,
                None => {
                    pos_map.insert(remote.id, index.characters.len());
                    index.characters.push(summary);
                }
            }
            index_dirty = true;
        }
    }

    // Push local-only characters (not seen in remote) to cloud.
    if push_local_only {
        for summary in &index.characters {
            if !seen_remote.contains(&summary.id)
                && let Some(character) = load_character(&summary.id)
            {
                log::info!("sync_all_with_cloud: pushing local-only {}", summary.id);
                if let Err(error) = push_to_cloud(&uid, &character).await {
                    log::warn!("Failed to push local-only character: {error:?}");
                    push_failures += 1;
                }
            }
        }
    }

    if index_dirty {
        save_index(&index);
        get_or_init_sync().index_version.update(|v| *v += 1);
    }

    if push_failures > 0 {
        Err(JsValue::from_str(&format!(
            "Failed to push {push_failures} character(s)"
        )))
    } else {
        Ok(())
    }
}
