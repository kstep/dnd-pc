use std::{collections::BTreeMap, fmt::Write};

use js_sys::{Date, Object, Reflect};
use reactive_stores::Store;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::{
    model::{Attribute, Character},
    rules::PendingInputs,
};

// --- Error ---

#[derive(Debug, Clone)]
pub enum AiError {
    NoWindow,
    Http { status: u16, body: String },
    Js(JsValue),
    Json(String),
    EmptyResponse,
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoWindow => write!(f, "no browser window"),
            Self::Http { status, body } => write!(f, "API error {status}: {body}"),
            Self::Js(error) => write!(f, "{error:?}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::EmptyResponse => write!(f, "no choices in API response"),
        }
    }
}

impl From<JsValue> for AiError {
    fn from(value: JsValue) -> Self {
        Self::Js(value)
    }
}

impl From<serde_json::Error> for AiError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

// --- Provider ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AiProvider {
    #[default]
    OpenAI,
    // TODO: Anthropic (requires CORS proxy or backend)
}

impl AiProvider {
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::OpenAI => "gpt-4o-mini",
        }
    }

    pub fn api_url(&self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com/v1/chat/completions",
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Self::OpenAI => "OpenAI",
        }
    }

    fn models_url(&self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com/v1/models",
        }
    }

    fn is_chat_model(&self, entry: &ModelEntry) -> bool {
        match self {
            Self::OpenAI => {
                entry.owned_by == "openai"
                    && (entry.id.starts_with("gpt-") || entry.id.starts_with("o"))
                    && !entry.id.contains("realtime")
                    && !entry.id.contains("audio")
                    && !entry.id.contains("search")
                    && !entry.id.contains("instruct")
            }
        }
    }
}

// --- HTTP helpers ---

/// Send an authenticated API request: serialize body, fetch, deserialize
/// response.
async fn api_request<T: Serialize, R: DeserializeOwned>(
    url: &str,
    api_key: &str,
    request: &T,
) -> Result<R, AiError> {
    let body_str = serde_json::to_string(request)?;
    let resp = api_fetch(url, "POST", api_key, Some(&body_str)).await?;
    let text = response_text(resp).await?;
    Ok(serde_json::from_str(&text)?)
}

/// Low-level authenticated fetch returning raw `web_sys::Response`.
/// Used by `api_request` and streaming endpoints.
async fn api_fetch(
    url: &str,
    method: &str,
    api_key: &str,
    body: Option<&str>,
) -> Result<web_sys::Response, AiError> {
    let opts = web_sys::RequestInit::new();
    opts.set_method(method);

    let headers = web_sys::Headers::new()?;
    headers.set("Authorization", &format!("Bearer {api_key}"))?;
    if let Some(body_str) = body {
        headers.set("Content-Type", "application/json")?;
        opts.set_body(&JsValue::from_str(body_str));
    }
    opts.set_headers(&headers);

    let request = web_sys::Request::new_with_str_and_init(url, &opts)?;

    let window = web_sys::window().ok_or(AiError::NoWindow)?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

    let resp: web_sys::Response = resp_value.dyn_into()?;

    if !resp.ok() {
        let status = resp.status();
        let body = JsFuture::from(resp.text()?)
            .await
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        return Err(AiError::Http { status, body });
    }

    Ok(resp)
}

async fn response_text(resp: web_sys::Response) -> Result<String, AiError> {
    JsFuture::from(resp.text()?)
        .await?
        .as_string()
        .ok_or(AiError::EmptyResponse)
}

// --- Models ---

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
    owned_by: String,
}

/// Fetch available chat models from the provider API.
pub async fn fetch_models(settings: &AiSettings) -> Result<Vec<String>, AiError> {
    let resp = api_fetch(
        settings.provider.models_url(),
        "GET",
        &settings.api_key,
        None,
    )
    .await?;
    let text = response_text(resp).await?;
    let parsed: ModelsResponse = serde_json::from_str(&text)?;

    let mut models: Vec<String> = parsed
        .data
        .into_iter()
        .filter(|entry| settings.provider.is_chat_model(entry))
        .map(|entry| entry.id)
        .collect();

    models.sort();
    Ok(models)
}

// --- Settings ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
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
        let date = Date::new_0();
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
        let mut out = String::with_capacity(512);
        let _ = write!(
            out,
            "Character: {}, Level {} {} {}",
            self.name, self.level, self.species, self.class_summary
        );
        if !self.history.is_empty() {
            let _ = write!(out, "\nBackstory: {}", self.history);
        }
        if !self.personality_traits.is_empty() {
            let _ = write!(out, "\nPersonality: {}", self.personality_traits);
        }
        if !self.ideals.is_empty() {
            let _ = write!(out, "\nIdeals: {}", self.ideals);
        }
        if !self.bonds.is_empty() {
            let _ = write!(out, "\nBonds: {}", self.bonds);
        }
        if !self.flaws.is_empty() {
            let _ = write!(out, "\nFlaws: {}", self.flaws);
        }
        if !self.notes.is_empty() {
            let notes = if self.notes.len() > 2000 {
                &self.notes[..self.notes.floor_char_boundary(2000)]
            } else {
                &self.notes
            };
            let _ = write!(out, "\nRecent notes: {notes}");
        }
        out
    }
}

// --- Story generation (streaming) ---

#[derive(Serialize)]
struct StreamingChatRequest<'a> {
    model: &'a str,
    stream: bool,
    messages: Vec<ChatMessage<'a>>,
}

const STORY_SYSTEM_PROMPT: &str = "\
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
) -> Result<String, AiError> {
    let user_content = format!(
        "{}\n\nPlayer's request: {}",
        context.to_prompt_text(),
        prompt
    );
    let request_body = StreamingChatRequest {
        model: &settings.model,
        stream: true,
        messages: vec![
            ChatMessage {
                role: "system",
                content: STORY_SYSTEM_PROMPT,
            },
            ChatMessage {
                role: "user",
                content: &user_content,
            },
        ],
    };
    let body_str = serde_json::to_string(&request_body)?;

    let resp = api_fetch(
        settings.provider.api_url(),
        "POST",
        &settings.api_key,
        Some(&body_str),
    )
    .await?;

    let body_stream = resp.body().ok_or(AiError::EmptyResponse)?;
    let reader: web_sys::ReadableStreamDefaultReader =
        body_stream.get_reader().dyn_into().map_err(JsValue::from)?;

    let mut full_text = String::new();
    let mut buffer = String::new();
    let decoder = web_sys::TextDecoder::new()?;

    loop {
        let result = JsFuture::from(reader.read()).await?;

        let done = Reflect::get(&result, &JsValue::from_str("done"))?
            .as_bool()
            .unwrap_or(true);

        if done {
            break;
        }

        let value = Reflect::get(&result, &JsValue::from_str("value"))?;

        let value_obj: Object = value.into();
        let chunk_text = decoder.decode_with_buffer_source(&value_obj)?;

        buffer.push_str(&chunk_text);

        // Process complete SSE lines from the buffer
        while let Some(newline_pos) = buffer.find('\n') {
            let line = &buffer[..newline_pos];
            let trimmed = line.trim();

            let should_break = if trimmed.is_empty() || trimmed.starts_with(':') {
                false
            } else if let Some(data) = trimmed.strip_prefix("data: ") {
                if data == "[DONE]" {
                    true
                } else {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data)
                        && let Some(content) = parsed["choices"][0]["delta"]["content"].as_str()
                    {
                        full_text.push_str(content);
                        on_chunk(content);
                    }
                    false
                }
            } else {
                false
            };

            buffer.drain(..newline_pos + 1);

            if should_break {
                break;
            }
        }
    }

    Ok(full_text)
}

// --- Non-streaming chat completion ---

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageContent,
}

#[derive(Deserialize)]
struct ChatMessageContent {
    content: String,
}

/// Non-streaming chat completion that parses the response content as JSON of
/// type `T`.
async fn chat_completion<T: DeserializeOwned>(
    settings: &AiSettings,
    system_prompt: &str,
    user_message: &str,
) -> Result<T, AiError> {
    let request_body = ChatRequest {
        model: &settings.model,
        messages: vec![
            ChatMessage {
                role: "system",
                content: system_prompt,
            },
            ChatMessage {
                role: "user",
                content: user_message,
            },
        ],
    };

    log::debug!("AI request to {}: system={system_prompt}", settings.model);
    log::debug!("AI user message:\n{user_message}");

    let response: ChatResponse = api_request(
        settings.provider.api_url(),
        &settings.api_key,
        &request_body,
    )
    .await?;

    let content = response
        .choices
        .into_iter()
        .next()
        .ok_or(AiError::EmptyResponse)?
        .message
        .content;

    log::debug!("AI response:\n{content}");

    // Strip markdown code fences if present (models without response_format)
    let json = content.trim();
    let json = json
        .strip_prefix("```json")
        .or_else(|| json.strip_prefix("```"))
        .and_then(|rest| rest.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(json);

    Ok(serde_json::from_str(json)?)
}

// --- Character concept ---

#[derive(Debug, Clone, Deserialize)]
pub struct CharacterConcept {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub species: String,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub subclass: Option<String>,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub abilities: [i32; 6],
    #[serde(default)]
    pub personality_traits: String,
    #[serde(default)]
    pub ideals: String,
    #[serde(default)]
    pub bonds: String,
    #[serde(default)]
    pub flaws: String,
    #[serde(default)]
    pub backstory: String,
}

// --- Character generation ---

pub async fn generate_character(
    settings: &AiSettings,
    description: &str,
    classes: &str,
    species: &str,
    backgrounds: &str,
) -> Result<CharacterConcept, AiError> {
    let system_prompt = "You are a D&D 5e character creator. Given a character description, \
        choose the best fitting options and create a complete character concept. \
        Write personality and backstory in the same language as the description. \
        Respond with valid JSON only, no markdown.";

    let user_message = format!(
        "Character description: {description}\n\n\
         Available species: {species}\n\
         Available classes: {classes}\n\
         Available backgrounds: {backgrounds}\n\n\
         Abilities must be a permutation of [15, 14, 13, 12, 10, 8] assigned to \
         [STR, DEX, CON, INT, WIS, CHA] in that order.\n\n\
         Respond with a JSON object with these fields:\n\
         - name: string\n\
         - species: string (exactly one of the available species)\n\
         - class: string (exactly one of the available classes)\n\
         - subclass: string or null (exactly one of the listed subclasses)\n\
         - background: string (exactly one of the available backgrounds)\n\
         - abilities: [STR, DEX, CON, INT, WIS, CHA] (array of 6 integers)\n\
         - personality_traits: string\n\
         - ideals: string\n\
         - bonds: string\n\
         - flaws: string\n\
         - backstory: string"
    );

    chat_completion(settings, system_prompt, &user_message).await
}

// --- Feature choices generation ---

pub struct PendingArgDescription {
    pub feature_name: String,
    pub feature_description: String,
    pub args_description: String,
}

/// Convert pending feature inputs into AI-readable descriptions of their ARG
/// parameters. Features with no active ARGs are skipped.
pub fn describe_pending_args(
    pending: &[PendingInputs],
    character: &Character,
) -> Vec<PendingArgDescription> {
    let is_arg = |var: &Attribute| -> Option<u8> {
        if let Attribute::Arg(n) = var {
            Some(*n)
        } else {
            None
        }
    };

    pending
        .iter()
        .filter_map(|inputs| {
            let mut args_lines = Vec::new();

            for expr in &inputs.exprs {
                let analysis = expr.analyze(character, is_arg);

                for &arg_index in &analysis.active_args {
                    if analysis.boolean_args.contains(&arg_index) {
                        args_lines
                            .push(format!("ARG.{arg_index}: integer, 0 or 1 (boolean choice)"));
                    } else {
                        args_lines.push(format!("ARG.{arg_index}: integer"));
                    }
                }

                if !analysis.active_args.is_empty() {
                    args_lines.push(format!("Expression: {expr}"));
                }
            }

            if args_lines.is_empty() {
                return None;
            }

            Some(PendingArgDescription {
                feature_name: inputs.feature_name.clone(),
                feature_description: inputs.feature_description.clone(),
                args_description: args_lines.join("\n"),
            })
        })
        .collect()
}

pub async fn generate_feature_choices(
    settings: &AiSettings,
    concept: &CharacterConcept,
    pending_args: &[PendingArgDescription],
) -> Result<BTreeMap<String, Vec<i32>>, AiError> {
    if pending_args.is_empty() {
        return Ok(BTreeMap::new());
    }

    let system_prompt = "You are a D&D 5e character builder. Given a character concept and \
        pending feature choices with ARG constraints, pick ARG values that best fit the character. \
        Respond with valid JSON only, no markdown.\n\n\
        Expression language reference:\n\
        - guard(condition, body): body executes only if condition is true\n\
        - in(x, min, max): true if min <= x <= max\n\
        - ARG.N: the Nth argument you must provide (0-indexed)\n\
        - Boolean ARGs (0 or 1) act as toggles to select options\n\
        - SKILL.XXXX.PROF: skill proficiency (0=none, 1=proficient)\n\
        - STR, DEX, CON, INT, WIS, CHA: ability scores\n\
        - The guard condition is a MANDATORY constraint: if it evaluates to false, \
        the feature application FAILS. Your ARG values MUST satisfy it.";

    let features_description: String = pending_args
        .iter()
        .map(|pending| {
            format!(
                "- \"{}\"\n  Description: {}\n  ARG constraints: {}",
                pending.feature_name, pending.feature_description, pending.args_description
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let user_message = format!(
        "Character concept:\n\
         Name: {name}\n\
         Species: {species}\n\
         Class: {class}\n\
         Background: {background}\n\
         Personality: {personality}\n\
         Backstory: {backstory}\n\n\
         Pending feature choices:\n{features}\n\n\
         Respond with a JSON object where each key is the feature name \
         and each value is an array of integer ARG values for that feature.",
        name = concept.name,
        species = concept.species,
        class = concept.class,
        background = concept.background,
        personality = concept.personality_traits,
        backstory = concept.backstory,
        features = features_description,
    );

    chat_completion(settings, system_prompt, &user_message).await
}
