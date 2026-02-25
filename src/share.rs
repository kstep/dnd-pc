use std::io::{Read, Write};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};

use crate::model::Character;

pub fn encode_character(character: &Character) -> String {
    let json = serde_json::to_string(character).expect("failed to serialize character");
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(json.as_bytes())
        .expect("failed to compress");
    let compressed = encoder.finish().expect("failed to finish compression");
    URL_SAFE_NO_PAD.encode(&compressed)
}

pub fn decode_character(data: &str) -> Option<Character> {
    let compressed = URL_SAFE_NO_PAD.decode(data).ok()?;
    let mut decoder = DeflateDecoder::new(&compressed[..]);
    let mut json = Vec::new();
    decoder.read_to_end(&mut json).ok()?;
    serde_json::from_slice(&json).ok()
}
