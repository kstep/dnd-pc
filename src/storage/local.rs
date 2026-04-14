use std::rc::Rc;

use gloo_storage::{LocalStorage, Storage};
use serde::Deserialize;
use uuid::Uuid;
use wasm_bindgen::{JsCast, prelude::*};

use crate::{
    ai::{AiSettings, Story},
    model::{ActiveEffects, Character, CharacterSummary, ClassLevel, format_classes},
    storage::migrate::deserialize_character_value,
};

/// Lightweight view over the character JSON blob. Only fields needed to
/// build a `CharacterSummary` — avoids full `Character` deserialization on
/// every list refresh.
#[derive(Deserialize)]
struct SummaryView {
    id: Uuid,
    #[serde(default)]
    updated_at: u64,
    #[serde(default)]
    shared: bool,
    #[serde(default)]
    identity: IdentityView,
}

#[derive(Deserialize, Default)]
struct IdentityView {
    #[serde(default)]
    name: String,
    #[serde(default)]
    classes: Vec<ClassLevel>,
}

impl From<SummaryView> for CharacterSummary {
    fn from(view: SummaryView) -> Self {
        let level = view
            .identity
            .classes
            .iter()
            .map(|c| c.level)
            .sum::<u32>()
            .max(1);
        CharacterSummary {
            id: view.id,
            name: view.identity.name,
            class: format_classes(&view.identity.classes),
            level,
            updated_at: view.updated_at,
            shared: view.shared,
        }
    }
}

const CHAR_KEY_PREFIX: &str = "dnd_pc_char_";
const LEGACY_INDEX_KEY: &str = "dnd_pc_index";
const LAST_SYNC_KEY: &str = "dnd_pc_last_sync";

pub fn character_key(id: &Uuid) -> String {
    format!("{CHAR_KEY_PREFIX}{id}")
}

fn effects_key(id: &Uuid) -> String {
    format!("dnd_pc_effects_{id}")
}

pub fn stories_key(id: &Uuid) -> String {
    format!("dnd_pc_stories_{id}")
}

pub fn load_effects(id: &Uuid) -> ActiveEffects {
    LocalStorage::get(effects_key(id)).unwrap_or_default()
}

pub fn save_effects(id: &Uuid, effects: &ActiveEffects) {
    if let Err(error) = LocalStorage::set(effects_key(id), effects) {
        log::error!("Failed to save effects: {error}");
    }
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
        .map(CharacterSummary::from)
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
            .and_then(|v| v.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|entry| entry.get("updated_at").and_then(|v| v.as_u64()))
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

pub fn load_character(id: &Uuid) -> Option<Character> {
    let key = character_key(id);
    if let Ok(character) = LocalStorage::get::<Character>(&key) {
        return Some(character);
    }
    // Fallback: migrate legacy format
    let raw = LocalStorage::raw().get_item(&key).ok()??;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    deserialize_character_value(value)
}

/// Pure save: write character to localStorage.
/// Does NOT touch `updated_at` or push to cloud.
pub fn save_character(character: &Character) {
    if let Err(error) = LocalStorage::set(character_key(&character.id), character) {
        log::error!("Failed to save character: {error}");
    }
}

/// Delete character blob, effects, and stories from localStorage.
pub fn delete_character_local_only(id: &Uuid) {
    LocalStorage::delete(character_key(id));
    LocalStorage::delete(effects_key(id));
    LocalStorage::delete(stories_key(id));
}

const LAST_EDITOR_TAB_KEY: &str = "dnd_pc_last_editor_tab";

pub fn load_last_editor_tab() -> String {
    LocalStorage::get(LAST_EDITOR_TAB_KEY).unwrap_or_else(|_| "stats".to_string())
}

pub fn save_last_editor_tab(tab: &str) {
    let _ = LocalStorage::set(LAST_EDITOR_TAB_KEY, tab);
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

/// Open a `.json` file picker, read the selected file, and call `on_character`
/// with the parsed [`Character`]. Shows a browser alert and logs on error.
pub fn pick_character_from_file<F: Fn(Character) + 'static>(on_character: F) {
    let on_character = Rc::new(on_character);
    let input: web_sys::HtmlInputElement = leptos::prelude::document()
        .create_element("input")
        .unwrap()
        .unchecked_into();

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
            Ok(reader) => reader,
            Err(error) => {
                log::error!("Failed to create FileReader: {error:?}");
                return;
            }
        };

        let reader_clone = reader.clone();
        let onload_js = Closure::once_into_js(move || {
            let result = match reader_clone.result() {
                Ok(result) => result,
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

    input.set_onchange(Some(onchange_js.unchecked_ref()));

    input.click();
}
