use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

use crate::model::{Character, CharacterIndex, DamageType};

const INDEX_KEY: &str = "dnd_pc_index";

fn character_key(id: &Uuid) -> String {
    format!("dnd_pc_char_{id}")
}

pub fn load_index() -> CharacterIndex {
    LocalStorage::get(INDEX_KEY).unwrap_or_default()
}

pub fn save_index(index: &CharacterIndex) {
    LocalStorage::set(INDEX_KEY, index).expect("failed to save index");
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

pub fn load_character(id: &Uuid) -> Option<Character> {
    let key = character_key(id);
    if let Ok(ch) = LocalStorage::get::<Character>(&key) {
        return Some(ch);
    }
    // Fallback: migrate legacy format
    let raw = LocalStorage::raw().get_item(&key).ok()??;
    let mut value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    migrate_v1(&mut value);
    serde_json::from_value(value).ok()
}

pub fn save_character(character: &mut Character) {
    character.touch();
    LocalStorage::set(character_key(&character.id), &*character).expect("failed to save character");

    let mut index = load_index();
    let summary = character.summary();
    if let Some(entry) = index.characters.iter_mut().find(|c| c.id == character.id) {
        *entry = summary;
    } else {
        index.characters.push(summary);
    }
    save_index(&index);
}

pub fn delete_character(id: &Uuid) {
    LocalStorage::delete(character_key(id));

    let mut index = load_index();
    index.characters.retain(|c| c.id != *id);
    save_index(&index);
}

/// Open a `.json` file picker, read the selected file, and call `on_character` with the parsed
/// [`Character`]. Shows a browser alert and logs on error.
pub fn pick_character_from_file<F: Fn(Character) + 'static>(on_character: F) {
    use std::rc::Rc;

    let on_character = Rc::new(on_character);
    let input: web_sys::HtmlInputElement = document()
        .create_element("input")
        .unwrap()
        .unchecked_into();

    input.set_type("file");
    input.set_accept(".json");

    let input_clone = input.clone();
    let closure = Closure::<dyn Fn()>::new(move || {
        let Some(files) = input_clone.files() else {
            return;
        };
        let Some(file) = files.get(0) else {
            return;
        };

        let reader = match web_sys::FileReader::new() {
            Ok(r) => r,
            Err(e) => {
                log::error!("Failed to create FileReader: {e:?}");
                return;
            }
        };

        let reader_clone = reader.clone();
        let on_character = Rc::clone(&on_character);
        let onload = Closure::<dyn Fn()>::new(move || {
            let result = match reader_clone.result() {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Failed to read file: {e:?}");
                    return;
                }
            };
            let Some(text) = result.as_string() else {
                log::error!("File result is not a string");
                return;
            };
            match serde_json::from_str::<Character>(&text) {
                Ok(character) => on_character(character),
                Err(e) => {
                    log::error!("Failed to parse character JSON: {e}");
                    window()
                        .alert_with_message(&format!("Invalid character file: {e}"))
                        .ok();
                }
            }
        });

        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        if let Err(e) = reader.read_as_text(&file) {
            log::error!("Failed to start reading file: {e:?}");
        }
    });

    input.set_onchange(Some(closure.as_ref().unchecked_ref()));
    closure.forget();

    input.click();
}
