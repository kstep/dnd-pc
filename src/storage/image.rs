use base64::{Engine, engine::general_purpose::STANDARD};
use js_sys::{Array, Promise, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, File, FileReader, ImageBitmap, ImageEncodeOptions, OffscreenCanvas,
    OffscreenCanvasRenderingContext2d,
};

const TARGET_WIDTH: f64 = 384.0;
const TARGET_HEIGHT: f64 = 512.0;
const WEBP_QUALITY: f64 = 0.80;

/// Loads the picked file, center-crops to 3:4, scales to 384×512, encodes as
/// WebP and returns a `data:image/webp;base64,…` URI. Returns the underlying
/// JS error verbatim — caller is responsible for surfacing it to the user.
pub async fn process_image_file(file: File) -> Result<String, JsValue> {
    let bitmap = blob_to_bitmap(&file).await?;
    bitmap_to_webp_data_uri(bitmap).await
}

/// Decodes a base64 image (any format the browser can decode), center-crops
/// to 3:4, scales to 384×512 and re-encodes as WebP. Returns the WebP data
/// URI. `createImageBitmap` sniffs magic bytes — we don't set a MIME type.
pub async fn process_image_base64(b64: &str) -> Result<String, JsValue> {
    let bytes = STANDARD
        .decode(b64.trim())
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let parts = Array::of1(&Uint8Array::from(bytes.as_slice()));
    let blob = Blob::new_with_u8_array_sequence(&parts)?;
    let bitmap = blob_to_bitmap(&blob).await?;
    bitmap_to_webp_data_uri(bitmap).await
}

async fn blob_to_bitmap(blob: &Blob) -> Result<ImageBitmap, JsValue> {
    let bitmap_promise: Promise = web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window"))?
        .create_image_bitmap_with_blob(blob)?;
    Ok(JsFuture::from(bitmap_promise).await?.unchecked_into())
}

async fn bitmap_to_webp_data_uri(bitmap: ImageBitmap) -> Result<String, JsValue> {
    let source_width = f64::from(bitmap.width());
    let source_height = f64::from(bitmap.height());
    let aspect_target = TARGET_WIDTH / TARGET_HEIGHT;
    let aspect_source = source_width / source_height;
    let (crop_w, crop_h) = if aspect_source > aspect_target {
        (source_height * aspect_target, source_height)
    } else {
        (source_width, source_width / aspect_target)
    };
    let crop_x = (source_width - crop_w) / 2.0;
    let crop_y = (source_height - crop_h) / 2.0;

    let canvas = OffscreenCanvas::new(TARGET_WIDTH as u32, TARGET_HEIGHT as u32)?;
    let ctx: OffscreenCanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d context"))?
        .unchecked_into();
    ctx.draw_image_with_image_bitmap_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
        &bitmap,
        crop_x,
        crop_y,
        crop_w,
        crop_h,
        0.0,
        0.0,
        TARGET_WIDTH,
        TARGET_HEIGHT,
    )?;
    bitmap.close();

    let options = ImageEncodeOptions::new();
    options.set_type("image/webp");
    options.set_quality(WEBP_QUALITY);
    let blob_promise: Promise = canvas.convert_to_blob_with_options(&options)?;
    let blob: Blob = JsFuture::from(blob_promise).await?.dyn_into()?;

    let reader = FileReader::new()?;
    let read_promise = Promise::new(&mut |resolve, reject| {
        let reject_for_start_error = reject.clone();
        let reject_for_err = reject.clone();
        let reader_clone_load = reader.clone();
        let reader_clone_err = reader.clone();
        let onload = wasm_bindgen::closure::Closure::once_into_js(move |_event: web_sys::Event| {
            match reader_clone_load.result() {
                Ok(value) => resolve.call1(&JsValue::NULL, &value).ok(),
                Err(error) => reject.call1(&JsValue::NULL, &error).ok(),
            };
        });
        reader.set_onload(Some(onload.unchecked_ref()));
        let onerror = wasm_bindgen::closure::Closure::once_into_js(move |event: web_sys::Event| {
            let payload = reader_clone_err
                .error()
                .map(JsValue::from)
                .unwrap_or_else(|| event.into());
            reject_for_err.call1(&JsValue::NULL, &payload).ok();
        });
        reader.set_onerror(Some(onerror.unchecked_ref()));
        if let Err(error) = reader.read_as_data_url(&blob) {
            reject_for_start_error.call1(&JsValue::NULL, &error).ok();
        }
    });
    let result = JsFuture::from(read_promise).await?;
    result
        .as_string()
        .ok_or_else(|| JsValue::from_str("FileReader did not return a string"))
}

pub fn monogram_initials(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut words = trimmed.split_whitespace();
    let first_word = words.next().unwrap_or("");
    if let Some(second_word) = words.next() {
        let mut initials = String::new();
        if let Some(letter) = first_word.chars().next() {
            initials.push(letter.to_uppercase().next().unwrap_or(letter));
        }
        if let Some(letter) = second_word.chars().next() {
            initials.push(letter.to_uppercase().next().unwrap_or(letter));
        }
        initials
    } else {
        first_word
            .chars()
            .take(2)
            .flat_map(|letter| letter.to_uppercase())
            .collect()
    }
}

pub fn monogram_hue(name: &str) -> u32 {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return 0;
    }
    // FNV-1a 32-bit
    let mut hash: u32 = 0x811c9dc5;
    for byte in trimmed.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x01000193);
    }
    // CSS hue degrees [0..360)
    hash % 360
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::{monogram_hue, monogram_initials};

    #[wasm_bindgen_test]
    fn initials_two_words() {
        assert_eq!(monogram_initials("Кир Эшворт"), "КЭ");
        assert_eq!(monogram_initials("Kir Ashworth"), "KA");
    }

    #[wasm_bindgen_test]
    fn initials_single_word() {
        assert_eq!(monogram_initials("Гэндальф"), "ГЭ");
        assert_eq!(monogram_initials("Bob"), "BO");
    }

    #[wasm_bindgen_test]
    fn initials_empty() {
        assert_eq!(monogram_initials(""), "");
        assert_eq!(monogram_initials("   "), "");
    }

    #[wasm_bindgen_test]
    fn initials_three_words_takes_two() {
        assert_eq!(monogram_initials("Foo Bar Baz"), "FB");
    }

    #[wasm_bindgen_test]
    fn hue_is_stable() {
        let first = monogram_hue("Кир Эшворт");
        let second = monogram_hue("Кир Эшворт");
        assert_eq!(first, second);
        assert!(first < 360);
    }

    #[wasm_bindgen_test]
    fn hue_differs_per_name() {
        assert_ne!(monogram_hue("Alice"), monogram_hue("Bob"));
    }

    #[wasm_bindgen_test]
    fn hue_empty_is_zero() {
        assert_eq!(monogram_hue(""), 0);
    }
}
