use std::io::{Read, Write};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::model::Character;

fn strip_for_sharing(character: &Character) -> Character {
    let mut character = character.clone();

    character.combat.death_save_successes = 0;
    character.combat.death_save_failures = 0;
    character.combat.hp_temp = 0;

    for feature in &mut character.features {
        feature.description.clear();
    }

    for fields in &mut character.fields.values_mut() {
        for field in fields {
            field.description.clear();
        }
    }

    for racial_trait in &mut character.racial_traits {
        racial_trait.description.clear();
    }

    if let Some(sc) = &mut character.spellcasting {
        for spell in &mut sc.spells {
            spell.description.clear();
        }
    }

    character
}

pub fn encode_character(character: &Character) -> String {
    let character = strip_for_sharing(character);
    let bytes = postcard::to_allocvec(&character).expect("failed to serialize character");
    let mut compressed = Vec::new();
    {
        let mut encoder = brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22);
        encoder.write_all(&bytes).expect("failed to compress");
    }
    let encoded = URL_SAFE_NO_PAD.encode(&compressed);

    log::info!(
        "share character: bytes={}, compressed={}, encoded={}, value={encoded}",
        bytes.len(),
        compressed.len(),
        encoded.len()
    );

    encoded
}

pub fn decode_character(data: &str) -> Option<Character> {
    let compressed = URL_SAFE_NO_PAD.decode(data).ok()?;
    let mut decoder = brotli::Decompressor::new(&compressed[..], 4096);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).ok()?;
    postcard::from_bytes(&bytes).ok()
}
