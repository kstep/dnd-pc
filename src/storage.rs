use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

use gloo_storage::{LocalStorage, Storage};
use leptos::{
    leptos_dom::helpers::{TimeoutHandle, set_timeout_with_handle},
    prelude::*,
};
use reactive_stores::Store;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{
    firebase,
    model::{ActiveEffects, Character, CharacterIndex, CharacterSummary, DamageType},
};

const INDEX_KEY: &str = "dnd_pc_index";
/// 2 s debounce — balances responsiveness vs. Firestore write-per-second cost.
const DEBOUNCE: Duration = Duration::from_millis(2000);

fn character_key(id: &Uuid) -> String {
    format!("dnd_pc_char_{id}")
}

fn effects_key(id: &Uuid) -> String {
    format!("dnd_pc_effects_{id}")
}

pub fn load_effects(id: &Uuid) -> ActiveEffects {
    LocalStorage::get(effects_key(id)).unwrap_or_default()
}

pub fn save_effects(id: &Uuid, effects: &ActiveEffects) {
    if let Err(error) = LocalStorage::set(effects_key(id), effects) {
        log::error!("Failed to save effects: {error}");
    }
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
    fn damage_type_from_name(s: &str) -> Option<DamageType> {
        match s.to_ascii_lowercase().as_str() {
            "acid" => Some(DamageType::Acid),
            "bludgeoning" => Some(DamageType::Bludgeoning),
            "cold" => Some(DamageType::Cold),
            "fire" => Some(DamageType::Fire),
            "force" => Some(DamageType::Force),
            "lightning" => Some(DamageType::Lightning),
            "necrotic" => Some(DamageType::Necrotic),
            "piercing" => Some(DamageType::Piercing),
            "poison" => Some(DamageType::Poison),
            "psychic" => Some(DamageType::Psychic),
            "radiant" => Some(DamageType::Radiant),
            "slashing" => Some(DamageType::Slashing),
            "thunder" => Some(DamageType::Thunder),
            _ => None,
        }
    }

    if let Some(weapons) = value
        .get_mut("equipment")
        .and_then(|e| e.get_mut("weapons"))
        .and_then(|w| w.as_array_mut())
    {
        for weapon in weapons {
            if let Some(dt) = weapon.get("damage_type").and_then(|v| v.as_str()) {
                let new_val = match damage_type_from_name(dt) {
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

/// Migrate FeatureValue::Die from string to { die, used } struct.
fn migrate_v4(value: &mut serde_json::Value) {
    if let Some(feature_data) = value
        .get_mut("feature_data")
        .and_then(|v| v.as_object_mut())
    {
        for entry in feature_data.values_mut() {
            if let Some(fields) = entry.get_mut("fields").and_then(|v| v.as_array_mut()) {
                for field in fields {
                    if let Some(die_str) = field.get("value").and_then(|v| v.get("Die"))
                        && let Some(s) = die_str.as_str()
                    {
                        let s = s.to_string();
                        field["value"] = serde_json::json!({ "Die": { "die": s, "used": 0 } });
                    }
                }
            }
        }
    }
}

/// Migrate broken "Languages" feature to per-species "Languages ({species})".
/// The global features index consolidation incorrectly merged all species'
/// Languages traits into one entry. Fix by looking up the species name and
/// replacing the feature + languages with the correct per-species version.
fn migrate_v5(value: &mut serde_json::Value) {
    // Check if character has a bare "Languages" feature
    let has_bare_languages = value
        .get("features")
        .and_then(|v| v.as_array())
        .is_some_and(|feats| {
            feats
                .iter()
                .any(|f| f.get("name").and_then(|n| n.as_str()) == Some("Languages"))
        });

    if !has_bare_languages {
        return;
    }

    // Get the species name
    let species = value
        .get("identity")
        .and_then(|id| id.get("species"))
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();

    if species.is_empty() {
        return;
    }

    // Map species → (correct language feature name, correct languages)
    let (lang_feat, correct_langs): (&str, &[&str]) = match species.as_str() {
        "Aasimar" => ("Languages (Aasimar)", &["Common", "Celestial"]),
        "Dragonborn" => ("Languages (Dragonborn)", &["Common", "Draconic"]),
        "Drow" | "High Elf" | "Wood Elf" | "Khoravar" => ("Languages (Elf)", &["Common", "Elvish"]),
        "Dwarf" => ("Languages (Dwarf)", &["Common", "Dwarvish"]),
        "Forest Gnome" | "Rock Gnome" => ("Languages (Gnome)", &["Common", "Gnomish"]),
        "Goliath" => ("Languages (Goliath)", &["Common", "Giant"]),
        "Halfling" => ("Languages (Halfling)", &["Common", "Halfling"]),
        "Human" | "Shifter" | "Warforged" | "Dhampir" => ("Languages (Human)", &["Common"]),
        "Orc" => ("Languages (Orc)", &["Common", "Orc"]),
        "Tiefling" | "Tiefling (Infernal)" => ("Languages (Tiefling)", &["Common", "Infernal"]),
        "Tiefling (Abyssal)" => ("Languages (Tiefling (Abyssal))", &["Common", "Abyssal"]),
        "Tiefling (Chthonic)" => ("Languages (Tiefling (Chthonic))", &["Common", "Sylvan"]),
        "Changeling" => ("Languages (Changeling)", &["Common", "Sylvan"]),
        "Kalashtar" => ("Languages (Kalashtar)", &["Common", "Quori"]),
        "Boggart" => ("Languages (Boggart)", &["Common", "Sylvan"]),
        "Faerie" => ("Languages (Faerie)", &["Common", "Sylvan"]),
        "Flamekin" => ("Languages (Flamekin)", &["Common", "Primordial"]),
        "Lorwyn Changeling" => ("Languages (Lorwyn Changeling)", &["Common", "Sylvan"]),
        "Rimekin" => ("Languages (Rimekin)", &["Common", "Primordial"]),
        _ => return,
    };

    // Remove "Languages" feature, add correct one
    if let Some(features) = value.get_mut("features").and_then(|v| v.as_array_mut()) {
        features.retain(|f| f.get("name").and_then(|n| n.as_str()) != Some("Languages"));
        features.push(serde_json::json!({"name": lang_feat}));
    }

    // Replace languages array with correct values
    let langs: Vec<serde_json::Value> = correct_langs
        .iter()
        .map(|l| serde_json::Value::String(l.to_string()))
        .collect();
    value["languages"] = serde_json::Value::Array(langs);
}

/// Set `applied: true` on all features that lack the field (old characters).
fn migrate_v6(value: &mut serde_json::Value) {
    if let Some(features) = value.get_mut("features").and_then(|v| v.as_array_mut()) {
        for feature in features {
            if feature.get("applied").is_none() {
                feature["applied"] = serde_json::Value::Bool(true);
            }
        }
    }
}

/// Migrate Feature entries: add `source` from FeatureData.source, remove
/// FeatureData.source. Convert FeatureSource::Class(String) to
/// Class(String, u32).
fn migrate_v7(value: &mut serde_json::Value) {
    // Build a map of feature_name → source from feature_data
    let mut sources: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(feature_data) = value.get("feature_data").and_then(|v| v.as_object()) {
        for (name, data) in feature_data {
            if let Some(source) = data.get("source") {
                let mut source = source.clone();
                // Convert Class(String) → Class(String, 1)
                if let Some(obj) = source.as_object_mut()
                    && let Some(class_val) = obj.get("Class")
                    && let Some(class_name) = class_val.as_str()
                {
                    let class_name = class_name.to_string();
                    obj.insert("Class".to_string(), serde_json::json!([class_name, 1]));
                }
                sources.insert(name.clone(), source);
            }
        }
    }

    // Add source to Feature entries that don't have one
    if let Some(features) = value.get_mut("features").and_then(|v| v.as_array_mut()) {
        for feature in features {
            if (feature.get("source").is_none()
                || feature.get("source") == Some(&serde_json::Value::Null))
                && let Some(name) = feature.get("name").and_then(|v| v.as_str())
            {
                if let Some(source) = sources.get(name) {
                    feature["source"] = source.clone();
                } else {
                    feature["source"] = serde_json::json!({"User": 0});
                }
            }
        }
    }

    // Remove source and rename args → inputs in feature_data entries
    if let Some(feature_data) = value
        .get_mut("feature_data")
        .and_then(|v| v.as_object_mut())
    {
        for (_, data) in feature_data.iter_mut() {
            if let Some(obj) = data.as_object_mut() {
                obj.remove("source");
                if let Some(args) = obj.remove("args") {
                    // Rename AssignArgs.values → AssignInputs.args inside each entry
                    let inputs = if let Some(arr) = args.as_array() {
                        let migrated: Vec<serde_json::Value> = arr
                            .iter()
                            .map(|entry| {
                                if let Some(obj) = entry.as_object()
                                    && let Some(values) = obj.get("values")
                                {
                                    serde_json::json!({"args": values})
                                } else {
                                    entry.clone()
                                }
                            })
                            .collect();
                        serde_json::Value::Array(migrated)
                    } else {
                        args
                    };
                    obj.insert("inputs".to_string(), inputs);
                }
            }
        }
    }
}

/// Move `inputs` from `feature_data[name].inputs` to `features[i].inputs`.
/// Each feature instance now carries its own inputs for stackable support.
fn migrate_v8(value: &mut serde_json::Value) {
    // Build a map: feature_name → VecDeque of inputs arrays
    let mut inputs_by_name: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    if let Some(feature_data) = value.get("feature_data").and_then(|v| v.as_object()) {
        for (name, data) in feature_data {
            if let Some(inputs) = data.get("inputs").and_then(|v| v.as_array())
                && !inputs.is_empty()
            {
                inputs_by_name.insert(name.clone(), inputs.clone());
            }
        }
    }

    // Distribute inputs to matching feature entries (in order)
    if let Some(features) = value.get_mut("features").and_then(|v| v.as_array_mut()) {
        for feature in features {
            if let Some(name) = feature.get("name").and_then(|v| v.as_str())
                && let Some(inputs) = inputs_by_name.get_mut(name)
                && !inputs.is_empty()
            {
                let input = inputs.remove(0);
                feature["inputs"] = serde_json::json!([input]);
            }
        }
    }

    // Remove inputs from feature_data entries
    if let Some(feature_data) = value
        .get_mut("feature_data")
        .and_then(|v| v.as_object_mut())
    {
        for (_, data) in feature_data.iter_mut() {
            if let Some(obj) = data.as_object_mut() {
                obj.remove("inputs");
            }
        }
    }
}

/// Deserialize a `serde_json::Value` into a `Character`, applying all
/// migrations. Used for cloud-fetched data.
pub fn deserialize_character_value(mut value: serde_json::Value) -> Option<Character> {
    migrate_v1(&mut value);
    migrate_v2(&mut value);
    migrate_v3(&mut value);
    migrate_v4(&mut value);
    migrate_v5(&mut value);
    migrate_v6(&mut value);
    migrate_v7(&mut value);
    migrate_v8(&mut value);
    serde_json::from_value(value).ok()
}

pub fn load_character(id: &Uuid) -> Option<Character> {
    let key = character_key(id);
    if let Ok(ch) = LocalStorage::get::<Character>(&key) {
        return Some(ch);
    }
    // Fallback: migrate legacy format
    let raw = LocalStorage::raw().get_item(&key).ok()??;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    deserialize_character_value(value)
}

fn upsert_index_entry(index: &mut CharacterIndex, summary: CharacterSummary) {
    index.characters.insert(summary.id, summary);
}

/// Pure save: write character to localStorage and update index.
/// Does NOT touch `updated_at` or push to cloud.
pub fn save_character(character: &Character) {
    LocalStorage::set(character_key(&character.id), character).expect("failed to save character");
    let summary = character.summary();
    update_index(|index| upsert_index_entry(index, summary));
}

/// Touch, save to localStorage, and schedule a debounced cloud push.
/// Used by non-reactive callers (import, copy, create).
pub fn save_and_sync_character(character: &mut Character) {
    character.touch();
    save_character(character);
    schedule_cloud_push(character);
}

pub fn delete_character(id: &Uuid) {
    LocalStorage::delete(character_key(id));

    let id = *id;
    update_index(|index| {
        index.characters.shift_remove(&id);
    });

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
            match serde_json::from_str(&text)
                .ok()
                .and_then(deserialize_character_value)
            {
                Some(character) => on_character(character),
                None => {
                    log::error!("Failed to parse character JSON");
                    window().alert_with_message("Invalid character file").ok();
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
    /// Set to `true` once the initial cloud sync completes (success or
    /// failure). Before this, auto-save skips `touch()` to preserve
    /// timestamps for accurate conflict resolution.
    sync_done: RwSignal<bool>,
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
    static DEBOUNCE_TIMERS: RefCell<HashMap<Uuid, TimeoutHandle>> = RefCell::new(HashMap::new());
    /// Cached character index to avoid repeated localStorage round-trips on every
    /// save. Lazily populated on first access; kept in sync with localStorage.
    static INDEX_CACHE: RefCell<Option<CharacterIndex>> = const { RefCell::new(None) };
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
            sync_done: RwSignal::new(false),
        })
    })
}

pub fn sync_status() -> ReadSignal<SyncStatus> {
    get_or_init_sync().status.read_only()
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

/// Reactive signal that becomes `true` once the initial cloud sync completes
/// (success, failure, or disabled). Before this, auto-save preserves existing
/// timestamps so sync can compare them accurately.
pub fn initial_sync_done() -> ReadSignal<bool> {
    get_or_init_sync().sync_done.read_only()
}

/// Set up auto-save and cloud sync pull for a character store.
/// Replaces the manual auto-save effect + `track_cloud_character` in layout.
pub fn setup_auto_save(store: Store<Character>) {
    let sync_done = initial_sync_done();
    let cloud_updating = RwSignal::new(false);

    // Auto-save effect
    Effect::new(move || {
        store.track();
        // Skip save when the store was just updated from a cloud pull.
        // Reset the flag here (not in the pull effect) so it works
        // regardless of whether Leptos flushes effects synchronously.
        if cloud_updating.get_untracked() {
            cloud_updating.update_untracked(|v| *v = false);
            return;
        }
        if sync_done.get() {
            // After initial sync: touch + save + cloud push
            store.update_untracked(|c| {
                c.touch();
                save_character(c);
                schedule_cloud_push(c);
            });
        } else {
            // Before initial sync: save only (preserve timestamp)
            save_character(&store.read_untracked());
        }
    });

    // Cloud sync pull: reload store when remote is newer.
    // Check the index timestamp first (cached in memory) to avoid full
    // Character deserialization when a different character was updated.
    let index_version = sync_index_version();
    Effect::new(move |previous: Option<u32>| {
        let (id, local_at) = {
            let character = store.read_untracked();
            (character.id, character.updated_at)
        };
        if previous.is_some() {
            let index_at = load_index()
                .characters
                .get(&id)
                .map(|c| c.updated_at)
                .unwrap_or(0);
            if index_at > local_at
                && let Some(character) = load_character(&id)
            {
                cloud_updating.update_untracked(|v| *v = true);
                store.set(character);
            }
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
            state.sync_done.set(true);
            return;
        }

        // Wait for auth state to settle (handles redirects, restored sessions, etc.)
        if let Some((uid, is_anon)) = firebase::wait_for_auth().await {
            log::info!("Auth settled: uid={uid}, anon={is_anon}");
            finish_sign_in(state, is_anon, SyncOp::for_anon(is_anon)).await;
            state.sync_done.set(true);
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
        state.sync_done.set(true);
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
    let char_key = character_key(&char_id);
    let char_id_str = char_id.to_string();

    // Cancel existing debounce timer for this character
    DEBOUNCE_TIMERS.with(|timers| {
        if let Some(handle) = timers.borrow_mut().remove(&char_id) {
            handle.clear();
        }
    });

    // Defer serialization: read raw JSON from localStorage when the timer fires,
    // parse directly to serde_json::Value, skipping Character deserialization.
    let Ok(handle) = set_timeout_with_handle(
        move || {
            DEBOUNCE_TIMERS.with(|timers| {
                timers.borrow_mut().remove(&char_id);
            });
            let state = get_or_init_sync();
            spawn_local(async move {
                let Some(uid) = firebase::current_uid() else {
                    return;
                };
                let Ok(Some(raw)) = LocalStorage::raw().get_item(&char_key) else {
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
        },
        DEBOUNCE,
    ) else {
        return;
    };
    DEBOUNCE_TIMERS.with(|timers| {
        timers.borrow_mut().insert(char_id, handle);
    });
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
    let mut index_dirty = false;
    let mut push_failures: u32 = 0;
    let mut seen_remote: HashSet<Uuid> = HashSet::with_capacity(remote_chars.len());

    for remote_value in remote_chars {
        let remote: Character = match deserialize_character_value(remote_value) {
            Some(character) => character,
            None => {
                log::warn!("Failed to deserialize remote character (migration failed)");
                continue;
            }
        };
        seen_remote.insert(remote.id);

        // Check local timestamp from the index (already in memory) to avoid
        // unnecessary full Character deserialization from localStorage.
        let local_updated_at = index
            .characters
            .get(&remote.id)
            .map(|c| c.updated_at)
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
            index.characters.insert(summary.id, summary);
            index_dirty = true;
        }
    }

    // Push local-only characters (not seen in remote) to cloud.
    if push_local_only {
        for summary in index.characters.values() {
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
