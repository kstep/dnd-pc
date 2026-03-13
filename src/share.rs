use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use js_sys::Uint8Array;
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_futures::JsFuture;

use crate::{model::Character, rules::RulesRegistry};

impl Character {
    fn strip_for_sharing(&mut self) {
        self.combat.death_save_successes = 0;
        self.combat.death_save_failures = 0;
        self.combat.hp_temp = 0;
    }
}

fn strip_for_sharing(character: &Character, registry: Option<&RulesRegistry>) -> Character {
    let mut character = character.clone();

    character.strip_for_sharing();
    if let Some(registry) = registry {
        registry.clear_from_registry(&mut character);
    } else {
        character.clear_all_labels();
    }

    character
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = CompressionStream)]
    type JsCompressionStream;

    #[wasm_bindgen(constructor, js_class = "CompressionStream")]
    fn new(format: &str) -> JsCompressionStream;

    #[wasm_bindgen(method, getter)]
    fn readable(this: &JsCompressionStream) -> web_sys::ReadableStream;

    #[wasm_bindgen(method, getter)]
    fn writable(this: &JsCompressionStream) -> web_sys::WritableStream;

    #[wasm_bindgen(js_name = DecompressionStream)]
    type JsDecompressionStream;

    #[wasm_bindgen(constructor, js_class = "DecompressionStream")]
    fn new(format: &str) -> JsDecompressionStream;

    #[wasm_bindgen(method, getter)]
    fn readable(this: &JsDecompressionStream) -> web_sys::ReadableStream;

    #[wasm_bindgen(method, getter)]
    fn writable(this: &JsDecompressionStream) -> web_sys::WritableStream;
}

async fn pipe_through_stream(data: &[u8], transform: &JsValue) -> Result<Vec<u8>, JsValue> {
    // Create a Response from the input data to get a ReadableStream
    let js_data = Uint8Array::from(data);
    let input_response = web_sys::Response::new_with_opt_buffer_source(Some(&js_data))?;
    let input_stream = input_response.body().ok_or("no body")?;

    // Pipe through the compression/decompression transform
    let output_stream =
        input_stream.pipe_through(transform.unchecked_ref::<web_sys::ReadableWritablePair>());

    // Read the result via another Response
    let output_response = web_sys::Response::new_with_opt_readable_stream(Some(&output_stream))?;
    let buf = JsFuture::from(output_response.array_buffer()?).await?;
    let array = Uint8Array::new(&buf);
    Ok(array.to_vec())
}

async fn compress(data: &[u8]) -> Result<Vec<u8>, JsValue> {
    let compressor = JsCompressionStream::new("deflate-raw");
    pipe_through_stream(data, compressor.as_ref()).await
}

async fn decompress(data: &[u8]) -> Result<Vec<u8>, JsValue> {
    let decompressor = JsDecompressionStream::new("deflate-raw");
    pipe_through_stream(data, decompressor.as_ref()).await
}

pub async fn encode_character(
    character: &Character,
    registry: Option<&RulesRegistry>,
) -> Option<String> {
    let character = strip_for_sharing(character, registry);
    let bytes = postcard::to_allocvec(&character).ok()?;
    let compressed = compress(&bytes).await.ok()?;
    let encoded = URL_SAFE_NO_PAD.encode(&compressed);

    log::info!(
        "share character: bytes={}, compressed={}, encoded={}",
        bytes.len(),
        compressed.len(),
        encoded.len()
    );

    Some(encoded)
}

pub async fn decode_character(data: &str) -> Option<Character> {
    let compressed = URL_SAFE_NO_PAD.decode(data).ok()?;
    let bytes = decompress(&compressed).await.ok()?;
    let character: Character = postcard::from_bytes(&bytes).ok()?;
    Some(character)
}

#[cfg(test)]
pub mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::model::Ability;

    #[wasm_bindgen_test]
    async fn encode_decode_roundtrip() {
        let ch = Character::test_character();
        let encoded = encode_character(&ch, None).await.expect("encode failed");
        let decoded = decode_character(&encoded).await.expect("decode failed");

        // Core identity preserved
        assert_eq!(decoded.id, ch.id);
        assert_eq!(decoded.identity.name, "Share Test");
        assert_eq!(decoded.identity.classes[0].class, "Bard");
        assert_eq!(decoded.identity.classes[0].level, 3);
        assert_eq!(decoded.ability_score(Ability::Charisma), 16);

        // Stripped fields should be zeroed
        assert_eq!(decoded.combat.death_save_successes, 0);
        assert_eq!(decoded.combat.death_save_failures, 0);
        assert_eq!(decoded.combat.hp_temp, 0);

        // Descriptions should be cleared
        assert!(decoded.features[0].description.is_empty());
    }

    #[wasm_bindgen_test]
    fn strip_zeros_death_saves_and_hp_temp() {
        let ch = Character::test_character();
        let stripped = strip_for_sharing(&ch, None);

        assert_eq!(stripped.combat.death_save_successes, 0);
        assert_eq!(stripped.combat.death_save_failures, 0);
        assert_eq!(stripped.combat.hp_temp, 0);

        // hp_current and hp_max should be preserved
        assert_eq!(stripped.combat.hp_current, 20);
        assert_eq!(stripped.combat.hp_max, 24);
    }

    #[wasm_bindgen_test]
    fn strip_clears_descriptions() {
        let ch = Character::test_character();
        let stripped = strip_for_sharing(&ch, None);

        // Feature descriptions cleared
        assert!(stripped.features[0].description.is_empty());

        // Spell descriptions cleared
        let spell_data = stripped
            .feature_data
            .get("Spellcasting (Bard)")
            .unwrap()
            .spells
            .as_ref()
            .unwrap();
        assert!(spell_data.spells[0].description.is_empty());

        // But spell name preserved
        assert_eq!(spell_data.spells[0].name, "Vicious Mockery");
    }

    #[wasm_bindgen_test]
    async fn decode_garbage_returns_none() {
        assert!(decode_character("not-valid-data!!!").await.is_none());
        assert!(decode_character("").await.is_none());
        assert!(decode_character("AAAA").await.is_none());
    }
}
