use std::rc::Rc;

use gloo_storage::{LocalStorage, Storage};
use serde::Deserialize;
use uuid::Uuid;
use wasm_bindgen::{JsCast, prelude::*};

use crate::{
    ai::{AiSettings, Story},
    model::{ActiveEffects, Avatar, Character, CharacterSummary, ClassLevel, format_classes},
    storage::{
        migrate::{self, deserialize_character_value},
        sync::{schedule_avatar_delete, schedule_avatar_push},
    },
};

/// Lightweight partial view over the avatar JSON blob. Reads only the
/// timestamp, avoiding the ~50-200KB data_uri deserialization.
#[derive(Deserialize, Default)]
struct AvatarView {
    #[serde(default)]
    updated_at: u64,
}

/// Lightweight view over the character JSON blob. Only fields needed to
/// build a `CharacterSummary` — avoids full `Character` deserialization on
/// every list refresh. Reads `identity` from `core.identity` (schema v4+);
/// falls back to top-level `identity` for un-migrated characters.
#[derive(Deserialize)]
struct SummaryView {
    id: Uuid,
    #[serde(default)]
    updated_at: u64,
    #[serde(default)]
    shared: bool,
    #[serde(default)]
    core: CoreView,
    #[serde(default)]
    identity: IdentityView,
    #[serde(default)]
    personality: PersonalityView,
}

#[derive(Deserialize, Default)]
struct CoreView {
    #[serde(default)]
    identity: IdentityView,
}

#[derive(Deserialize, Default)]
struct IdentityView {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    classes: Vec<ClassLevel>,
}

#[derive(Deserialize, Default)]
struct PersonalityView {
    #[serde(default)]
    name: Option<String>,
}

impl From<SummaryView> for CharacterSummary {
    fn from(view: SummaryView) -> Self {
        let classes = if !view.core.identity.classes.is_empty() {
            view.core.identity.classes
        } else {
            view.identity.classes
        };
        let name = view
            .personality
            .name
            .as_deref()
            .or(view.identity.name.as_deref())
            .unwrap_or_default()
            .to_owned();
        let level = classes.iter().map(|cl| cl.level).sum::<u32>().max(1);
        CharacterSummary {
            id: view.id,
            name,
            class: format_classes(&classes),
            level,
            updated_at: view.updated_at,
            avatar_updated_at: None,
            shared: view.shared,
        }
    }
}

const CHAR_KEY_PREFIX: &str = "dnd_pc_char_";
const LEGACY_INDEX_KEY: &str = "dnd_pc_index";
const LAST_SYNC_KEY: &str = "dnd_pc_last_sync";
const LAST_SYNC_AVATARS_KEY: &str = "dnd_pc_last_sync_avatars";

pub fn character_key(id: &Uuid) -> String {
    format!("{CHAR_KEY_PREFIX}{id}")
}

fn effects_key(id: &Uuid) -> String {
    format!("dnd_pc_effects_{id}")
}

pub fn stories_key(id: &Uuid) -> String {
    format!("dnd_pc_stories_{id}")
}

pub(super) fn avatar_key(id: &Uuid) -> String {
    format!("dnd_pc_avatar_{id}")
}

pub fn load_effects(id: &Uuid) -> ActiveEffects {
    LocalStorage::get(effects_key(id)).unwrap_or_default()
}

pub fn save_effects(id: &Uuid, effects: &ActiveEffects) {
    if let Err(error) = LocalStorage::set(effects_key(id), effects) {
        log::error!("Failed to save effects: {error}");
    }
}

pub fn load_avatar(id: &Uuid) -> Option<Avatar> {
    LocalStorage::get::<Avatar>(avatar_key(id))
        .ok()
        .filter(|avatar| !avatar.is_empty())
}

/// Read only the avatar's timestamp without parsing the data_uri.
pub fn load_avatar_timestamp(id: &Uuid) -> Option<u64> {
    LocalStorage::raw()
        .get_item(&avatar_key(id))
        .ok()
        .flatten()
        .and_then(|blob| serde_json::from_str::<AvatarView>(&blob).ok())
        .map(|view| view.updated_at)
        .filter(|&ts| ts > 0)
}

pub fn save_avatar(id: &Uuid, avatar: &Avatar) {
    if let Err(error) = LocalStorage::set(avatar_key(id), avatar) {
        log::error!("Failed to save avatar: {error}");
        return;
    }
    schedule_avatar_push(*id);
}

/// Save avatar to localStorage without scheduling a cloud push.
/// Used by sync pull paths to avoid re-uploading just-pulled data.
pub fn save_avatar_quiet(id: &Uuid, avatar: &Avatar) {
    if let Err(error) = LocalStorage::set(avatar_key(id), avatar) {
        log::error!("Failed to save avatar: {error}");
    }
}

/// Remove avatar from localStorage without scheduling a cloud delete.
/// Used by sync pull paths to propagate remote deletion without echo.
pub fn remove_avatar_quiet(id: &Uuid) {
    LocalStorage::delete(avatar_key(id));
}

pub fn remove_avatar(id: &Uuid) {
    LocalStorage::delete(avatar_key(id));
    schedule_avatar_delete(*id);
}

/// Scan localStorage for all `dnd_pc_char_*` keys and return their summaries.
/// Single source of truth — no separate index. Uses a lightweight partial
/// deserialization (`SummaryView`) to avoid parsing the full character blob.
pub fn load_all_summaries() -> Vec<CharacterSummary> {
    let raw = LocalStorage::raw();
    let len = raw.length().unwrap_or(0);
    (0..len)
        .filter_map(|i| raw.key(i).ok().flatten())
        .filter(|key| key.starts_with(CHAR_KEY_PREFIX))
        .filter_map(|key| raw.get_item(&key).ok().flatten())
        .filter_map(|value| serde_json::from_str::<SummaryView>(&value).ok())
        .map(|view| {
            let mut summary = CharacterSummary::from(view);
            let key = avatar_key(&summary.id);
            summary.avatar_updated_at = raw
                .get_item(&key)
                .ok()
                .flatten()
                .and_then(|blob| serde_json::from_str::<AvatarView>(&blob).ok())
                .map(|view| view.updated_at)
                .filter(|&ts| ts > 0);
            summary
        })
        .collect()
}

pub fn load_last_sync() -> u64 {
    if let Ok(value) = LocalStorage::get::<u64>(LAST_SYNC_KEY) {
        return value;
    }
    // One-time migration: seed last_sync from the legacy index, then drop it.
    if let Ok(legacy) = LocalStorage::get::<serde_json::Value>(LEGACY_INDEX_KEY) {
        let seed = legacy
            .get("characters")
            .and_then(|value| value.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry.get("updated_at").and_then(|value| value.as_u64()))
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let _ = LocalStorage::set(LAST_SYNC_KEY, seed);
        LocalStorage::delete(LEGACY_INDEX_KEY);
        return seed;
    }
    0
}

pub fn save_last_sync(value: u64) {
    if let Err(error) = LocalStorage::set(LAST_SYNC_KEY, value) {
        log::error!("Failed to save last_sync: {error}");
    }
}

pub fn load_last_sync_avatars() -> u64 {
    LocalStorage::get::<u64>(LAST_SYNC_AVATARS_KEY).unwrap_or(0)
}

pub fn save_last_sync_avatars(value: u64) {
    if let Err(error) = LocalStorage::set(LAST_SYNC_AVATARS_KEY, value) {
        log::error!("Failed to save last_sync_avatars: {error}");
    }
}

pub fn load_character(id: &Uuid) -> Option<Character> {
    let key = character_key(id);
    let raw = LocalStorage::raw().get_item(&key).ok()??;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    // Direct typed deserialize defaults `core` for pre-v4 schemas, dropping
    // top-level identity/features — always migrate.
    deserialize_character_value(value)
}

/// Load the character blob for `id` from localStorage as a migrated
/// `serde_json::Value`. Skips the typed `Character` deserialize round-trip
/// used by `load_character` — the sync layer only needs the `Value` shape.
pub fn load_character_value(id: &Uuid) -> Option<serde_json::Value> {
    let key = character_key(id);
    let raw = LocalStorage::raw().get_item(&key).ok()??;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some(migrate::migrate_value(value))
}

/// Save a character as JSON to localStorage from a `serde_json::Value`.
/// Returns true on success. Logs and returns false on serialize or storage
/// error. Mirrors `load_character_value` for the write direction.
pub fn save_character_value(id: &Uuid, value: &serde_json::Value) -> bool {
    let serialized = match serde_json::to_string(value) {
        Ok(serialized) => serialized,
        Err(error) => {
            log::warn!("Failed to serialize character {id}: {error}");
            return false;
        }
    };
    if let Err(error) = LocalStorage::raw().set_item(&character_key(id), &serialized) {
        log::warn!("Failed to save character {id}: {error:?}");
        return false;
    }
    true
}

/// Pure save: write character to localStorage.
/// Does NOT touch `updated_at` or push to cloud.
pub fn save_character(character: &Character) {
    if let Err(error) = LocalStorage::set(character_key(&character.id), character) {
        log::error!("Failed to save character: {error}");
    }
}

/// Delete character blob, effects, stories, and avatar from localStorage.
pub fn delete_character_local_only(id: &Uuid) {
    LocalStorage::delete(character_key(id));
    LocalStorage::delete(effects_key(id));
    LocalStorage::delete(stories_key(id));
    LocalStorage::delete(avatar_key(id));
}

const LAST_EDITOR_TAB_KEY: &str = "dnd_pc_last_editor_tab";

pub fn load_last_editor_tab() -> String {
    LocalStorage::get(LAST_EDITOR_TAB_KEY).unwrap_or_else(|_| "stats".to_string())
}

pub fn save_last_editor_tab(tab: &str) {
    let _ = LocalStorage::set(LAST_EDITOR_TAB_KEY, tab);
}

const PERSONALITY_EXPANDED_KEY: &str = "dnd_pc_personality_expanded";

pub fn load_personality_expanded() -> Option<bool> {
    LocalStorage::get(PERSONALITY_EXPANDED_KEY).ok()
}

pub fn save_personality_expanded(expanded: bool) {
    let _ = LocalStorage::set(PERSONALITY_EXPANDED_KEY, expanded);
}

const AI_SETTINGS_KEY: &str = "dnd_pc_ai_settings";

pub fn load_ai_settings() -> AiSettings {
    LocalStorage::get(AI_SETTINGS_KEY).unwrap_or_default()
}

pub fn save_ai_settings(settings: &AiSettings) {
    if let Err(error) = LocalStorage::set(AI_SETTINGS_KEY, settings) {
        log::error!("Failed to save AI settings: {error}");
    }
}

pub fn load_stories(id: &Uuid) -> Vec<Story> {
    LocalStorage::get(stories_key(id)).unwrap_or_default()
}

pub fn save_stories(id: &Uuid, stories: &[Story]) {
    if let Err(error) = LocalStorage::set(stories_key(id), stories) {
        log::error!("Failed to save stories: {error}");
    }
}

/// Opens a file picker with the given accept filter and invokes `callback`
/// with the picked `File`. The callback runs synchronously inside the
/// `<input>`'s `onchange` — for async work, use `spawn_local` inside.
pub fn pick_file<F: Fn(web_sys::File) + 'static>(accept: &str, callback: F) {
    let callback = Rc::new(callback);
    let input: web_sys::HtmlInputElement = leptos::prelude::document()
        .create_element("input")
        .unwrap()
        .unchecked_into();
    input.set_type("file");
    input.set_accept(accept);

    let input_clone = input.clone();
    let onchange_js = Closure::once_into_js(move || {
        let Some(files) = input_clone.files() else {
            return;
        };
        let Some(file) = files.get(0) else {
            return;
        };
        callback(file);
    });
    input.set_onchange(Some(onchange_js.unchecked_ref()));
    input.click();
}

/// Open a `.json` file picker, read the selected file, and call `on_character`
/// with the parsed [`Character`]. Shows a browser alert and logs on error.
pub fn pick_character_from_file<F: Fn(Character) + 'static>(on_character: F) {
    let on_character = Rc::new(on_character);
    // `.txt` is accepted because the Web Share export path uses a `.txt`
    // suffix to pass Chrome Android's shareable-file allowlist.
    pick_file(".json,.txt,application/json,text/plain", move |file| {
        let on_character = on_character.clone();
        let reader = match web_sys::FileReader::new() {
            Ok(reader) => reader,
            Err(error) => {
                log::error!("Failed to create FileReader: {error:?}");
                return;
            }
        };
        let reader_clone = reader.clone();
        let onload_js = Closure::once_into_js(move || {
            let result = match reader_clone.result() {
                Ok(value) => value,
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
                    leptos::prelude::window()
                        .alert_with_message("Invalid character file")
                        .ok();
                }
            }
        });
        reader.set_onload(Some(onload_js.unchecked_ref()));
        if let Err(error) = reader.read_as_text(&file) {
            log::error!("Failed to start reading file: {error:?}");
        }
    });
}

#[cfg(test)]
mod avatar_tests {
    use uuid::Uuid;
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::{
        Avatar, delete_character_local_only, load_all_summaries, load_avatar,
        load_avatar_timestamp, remove_avatar, save_avatar, save_character,
    };
    use crate::model::Character;

    #[wasm_bindgen_test]
    fn save_load_round_trip() {
        let id = Uuid::new_v4();
        let avatar = Avatar {
            id,
            data_uri: "data:image/webp;base64,AQID".into(),
            updated_at: 1_700_000_000,
        };
        save_avatar(&id, &avatar);
        let loaded = load_avatar(&id).expect("avatar must load");
        assert_eq!(loaded, avatar);
        remove_avatar(&id);
    }

    #[wasm_bindgen_test]
    fn missing_avatar_returns_none() {
        let id = Uuid::new_v4();
        assert!(load_avatar(&id).is_none());
    }

    #[wasm_bindgen_test]
    fn remove_clears_storage() {
        let id = Uuid::new_v4();
        let avatar = Avatar {
            id,
            data_uri: "data:image/webp;base64,AQID".into(),
            updated_at: 1,
        };
        save_avatar(&id, &avatar);
        remove_avatar(&id);
        assert!(load_avatar(&id).is_none());
    }

    #[wasm_bindgen_test]
    fn save_avatar_reflects_in_summary() {
        let mut character = Character::default();
        character.id = uuid::Uuid::new_v4();
        save_character(&character);

        let avatar = Avatar {
            id: character.id,
            data_uri: "data:image/webp;base64,AQID".into(),
            updated_at: 42,
        };
        save_avatar(&character.id, &avatar);

        let summary = load_all_summaries()
            .into_iter()
            .find(|summary| summary.id == character.id)
            .expect("summary must exist");
        assert_eq!(summary.avatar_updated_at, Some(42));

        remove_avatar(&character.id);
        let summary = load_all_summaries()
            .into_iter()
            .find(|summary| summary.id == character.id)
            .expect("summary must exist");
        assert_eq!(summary.avatar_updated_at, None);

        // Cleanup
        delete_character_local_only(&character.id);
    }

    #[wasm_bindgen_test]
    fn timestamp_missing_returns_none() {
        let id = Uuid::new_v4();
        assert!(load_avatar_timestamp(&id).is_none());
    }

    #[wasm_bindgen_test]
    fn timestamp_round_trip() {
        let id = Uuid::new_v4();
        let avatar = Avatar {
            id,
            data_uri: "data:image/webp;base64,AQID".into(),
            updated_at: 12345,
        };
        save_avatar(&id, &avatar);
        assert_eq!(load_avatar_timestamp(&id), Some(12345));
        remove_avatar(&id);
    }

    #[wasm_bindgen_test]
    fn timestamp_zero_treated_as_none() {
        // Avatar with updated_at = 0 (default) should be filtered out as "no avatar"
        let id = Uuid::new_v4();
        let avatar = Avatar {
            id,
            data_uri: "data:image/webp;base64,AQID".into(),
            updated_at: 0,
        };
        save_avatar(&id, &avatar);
        assert_eq!(load_avatar_timestamp(&id), None);
        remove_avatar(&id);
    }
}
