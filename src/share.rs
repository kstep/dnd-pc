use std::io::{Read, Write};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::model::Character;

pub fn encode_character(character: &Character) -> String {
    let bytes = rmp_serde::to_vec(character).expect("failed to serialize character");
    let mut compressed = Vec::new();
    {
        let mut encoder = brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22);
        encoder.write_all(&bytes).expect("failed to compress");
    }
    URL_SAFE_NO_PAD.encode(&compressed)
}

pub fn decode_character(data: &str) -> Option<Character> {
    let compressed = URL_SAFE_NO_PAD.decode(data).ok()?;
    let mut decoder = brotli::Decompressor::new(&compressed[..], 4096);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).ok()?;
    rmp_serde::from_slice(&bytes).ok()
}
