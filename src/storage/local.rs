use std::{cell::RefCell, rc::Rc};

use gloo_storage::{LocalStorage, Storage};
use uuid::Uuid;
use wasm_bindgen::{JsCast, prelude::*};

use crate::{
    ai::{AiSettings, Story},
    model::{ActiveEffects, Character, CharacterIndex, CharacterSummary},
    storage::migrate::deserialize_character_value,
};

const INDEX_KEY: &str = "dnd_pc_index";

pub fn character_key(id: &Uuid) -> String {
    format!("dnd_pc_char_{id}")
}

fn effects_key(id: &Uuid) -> String {
    format!("dnd_pc_effects_{id}")
}

pub fn stories_key(id: &Uuid) -> String {
    format!("dnd_pc_stories_{id}")
}

thread_local! {
    /// Cached character index to avoid repeated localStorage round-trips on every
    /// save. Lazily populated on first access; kept in sync with localStorage.
    static INDEX_CACHE: RefCell<Option<CharacterIndex>> = const { RefCell::new(None) };
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
    if let Err(error) = LocalStorage::set(INDEX_KEY, index) {
        log::error!("Failed to save index: {error}");
        return;
    }
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
        if let Err(error) = LocalStorage::set(INDEX_KEY, &*index) {
            log::error!("Failed to save index: {error}");
        }
    });
}

fn upsert_index_entry(index: &mut CharacterIndex, summary: CharacterSummary) {
    index.characters.insert(summary.id, summary);
}

/// Update the index with a summary and persist to localStorage.
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

/// Pure save: write character to localStorage and update index.
/// Does NOT touch `updated_at` or push to cloud.
pub fn save_character(character: &Character) {
    if let Err(error) = LocalStorage::set(character_key(&character.id), character) {
        log::error!("Failed to save character: {error}");
        return;
    }
    let summary = character.summary();
    update_index(|index| upsert_index_entry(index, summary));
}

/// Delete character from localStorage and index without cloud delete.
pub fn delete_character_local_only(id: &Uuid) {
    LocalStorage::delete(character_key(id));
    LocalStorage::delete(stories_key(id));
    let id = *id;
    update_index(|index| {
        index.characters.shift_remove(&id);
    });
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
