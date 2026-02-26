use gloo_storage::{LocalStorage, Storage};
use uuid::Uuid;

use crate::model::{Character, CharacterIndex};

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

pub fn load_character(id: &Uuid) -> Option<Character> {
    LocalStorage::get(character_key(id)).ok()
}

pub fn save_character(character: &Character) {
    let mut character = character.clone();
    character.touch();
    LocalStorage::set(character_key(&character.id), &character).expect("failed to save character");

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
