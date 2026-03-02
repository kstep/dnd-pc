use gloo_storage::{LocalStorage, Storage};
use uuid::Uuid;

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
