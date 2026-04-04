use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

// --- Provider ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AiProvider {
    #[default]
    OpenAI,
    // TODO: Anthropic (requires CORS proxy or backend)
}

impl AiProvider {
    pub fn default_model(self) -> &'static str {
        match self {
            Self::OpenAI => "gpt-4o-mini",
        }
    }

    pub fn api_url(self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com/v1/chat/completions",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::OpenAI => "OpenAI",
        }
    }
}

// --- Settings ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiSettings {
    pub provider: AiProvider,
    pub api_key: String,
    pub model: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        let provider = AiProvider::default();
        Self {
            model: provider.default_model().to_string(),
            api_key: String::new(),
            provider,
        }
    }
}

impl AiSettings {
    pub fn has_api_key(&self) -> bool {
        !self.api_key.trim().is_empty()
    }
}

// --- Story ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Story {
    pub id: Uuid,
    pub title: String,
    pub prompt: String,
    pub content: String,
    pub created_at: String,
}

impl Story {
    pub fn new(title: String, prompt: String, content: String) -> Self {
        let date = js_sys::Date::new_0();
        Self {
            id: Uuid::new_v4(),
            title,
            prompt,
            content,
            created_at: date.to_iso_string().as_string().unwrap_or_default(),
        }
    }

    /// Format created_at as a short date string (e.g. "2026-04-04").
    pub fn short_date(&self) -> &str {
        self.created_at.get(..10).unwrap_or(&self.created_at)
    }
}

// --- Character context for prompts ---

pub struct CharacterContext {
    pub name: String,
    pub species: String,
    pub class_summary: String,
    pub level: u32,
    pub history: String,
    pub personality_traits: String,
    pub ideals: String,
    pub bonds: String,
    pub flaws: String,
    pub notes: String,
}

impl CharacterContext {
    pub fn to_prompt_text(&self) -> String {
        let mut parts = vec![format!(
            "Character: {}, Level {} {} {}",
            self.name, self.level, self.species, self.class_summary
        )];
        if !self.history.is_empty() {
            parts.push(format!("Backstory: {}", self.history));
        }
        if !self.personality_traits.is_empty() {
            parts.push(format!("Personality: {}", self.personality_traits));
        }
        if !self.ideals.is_empty() {
            parts.push(format!("Ideals: {}", self.ideals));
        }
        if !self.bonds.is_empty() {
            parts.push(format!("Bonds: {}", self.bonds));
        }
        if !self.flaws.is_empty() {
            parts.push(format!("Flaws: {}", self.flaws));
        }
        if !self.notes.is_empty() {
            let notes = if self.notes.len() > 2000 {
                &self.notes[..2000]
            } else {
                &self.notes
            };
            parts.push(format!("Recent notes: {notes}"));
        }
        parts.join("\n")
    }
}

// --- Story generation ---

const SYSTEM_PROMPT: &str = "\
You are a creative D&D storyteller. Write a short story about what \
the character did between game sessions, based on their details and \
the player's prompt. Write in the same language as the player's prompt.";

/// Generate a story by streaming from the OpenAI API.
///
/// `on_chunk` is called with each text fragment as it arrives.
/// Returns the complete generated text, or an error message.
pub async fn generate_story(
    settings: &AiSettings,
    context: &CharacterContext,
    prompt: &str,
    on_chunk: impl Fn(&str),
) -> Result<String, String> {
    let body = serde_json::json!({
        "model": settings.model,
        "stream": true,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": format!("{}\n\nPlayer's request: {}", context.to_prompt_text(), prompt) },
        ]
    });

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body.to_string()));

    let headers = web_sys::Headers::new().map_err(|error| format!("{error:?}"))?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|error| format!("{error:?}"))?;
    headers
        .set("Authorization", &format!("Bearer {}", settings.api_key))
        .map_err(|error| format!("{error:?}"))?;
    opts.set_headers(&headers);

    let request = web_sys::Request::new_with_str_and_init(settings.provider.api_url(), &opts)
        .map_err(|error| format!("{error:?}"))?;

    let window = web_sys::window().ok_or("no window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|error| format!("fetch failed: {error:?}"))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "response is not a Response")?;

    if !resp.ok() {
        let status = resp.status();
        let text = JsFuture::from(resp.text().map_err(|error| format!("{error:?}"))?)
            .await
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        return Err(format!("API error {status}: {text}"));
    }

    let body_stream = resp.body().ok_or("response has no body")?;
    let reader: web_sys::ReadableStreamDefaultReader = body_stream
        .get_reader()
        .dyn_into()
        .map_err(|_| "failed to get reader")?;

    let mut full_text = String::new();
    let mut buffer = String::new();
    let decoder = web_sys::TextDecoder::new().map_err(|error| format!("{error:?}"))?;

    loop {
        let result = JsFuture::from(reader.read())
            .await
            .map_err(|error| format!("read error: {error:?}"))?;

        let done = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
            .map_err(|error| format!("{error:?}"))?
            .as_bool()
            .unwrap_or(true);

        if done {
            break;
        }

        let value = js_sys::Reflect::get(&result, &JsValue::from_str("value"))
            .map_err(|error| format!("{error:?}"))?;

        let value_obj: js_sys::Object = value.into();
        let chunk_text = decoder
            .decode_with_buffer_source(&value_obj)
            .map_err(|error| format!("{error:?}"))?;

        buffer.push_str(&chunk_text);

        // Process complete SSE lines from the buffer
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data)
                    && let Some(content) = parsed["choices"][0]["delta"]["content"].as_str()
                {
                    full_text.push_str(content);
                    on_chunk(content);
                }
            }
        }
    }

    Ok(full_text)
}
