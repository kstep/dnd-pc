use std::{collections::BTreeMap, fmt::Write};

use js_sys::{Date, Object, Reflect};
use reactive_stores::Store;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::{
    expr::{self, BinOp, BlockIndex, Cmp, Interpreter, IterStack},
    firebase,
    hooks::join_iter,
    model::{Ability, Attribute, AttributeGroup, Character, Op, Skill},
    rules::{FeatureDefinition, PendingInputs},
};

/// Hosted proxy base URL injected at build time (`PROXY_URL` env). When set,
/// `AiProvider::Proxy` becomes the default and hosted AI is available.
pub const PROXY_BASE: Option<&str> = option_env!("PROXY_URL");

// --- Error ---

#[derive(Debug, Clone)]
pub enum AiError {
    NoWindow,
    Http {
        status: u16,
        body: String,
    },
    Js(JsValue),
    Json(String),
    EmptyResponse,
    /// Proxy mode: no Firebase user / token. Caller should prompt the user
    /// to sign in with Google or switch to BYOK.
    AuthRequired,
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoWindow => write!(f, "no browser window"),
            Self::Http { status, body } => write!(f, "API error {status}: {body}"),
            Self::Js(error) => write!(f, "{error:?}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::EmptyResponse => write!(f, "no choices in API response"),
            Self::AuthRequired => write!(f, "sign-in required"),
        }
    }
}

impl From<firebase::FirebaseError> for AiError {
    fn from(error: firebase::FirebaseError) -> Self {
        Self::Json(format!("{error:?}"))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiProvider {
    OpenAI,
    /// Hosted proxy — key lives server-side, auth via Firebase Google token.
    Proxy,
}

impl Default for AiProvider {
    /// Prefer the hosted proxy when it's available at build time; otherwise
    /// fall back to the direct OpenAI provider.
    fn default() -> Self {
        if Self::Proxy.is_available() {
            Self::Proxy
        } else {
            Self::OpenAI
        }
    }
}

impl AiProvider {
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::OpenAI | Self::Proxy => "gpt-4",
        }
    }

    pub fn default_image_model(&self) -> &'static str {
        match self {
            Self::OpenAI | Self::Proxy => "gpt-image-1",
        }
    }

    fn base_url(&self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com",
            Self::Proxy => PROXY_BASE.unwrap_or(""),
        }
    }

    pub fn api_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url())
    }

    pub fn images_url(&self) -> String {
        format!("{}/v1/images/generations", self.base_url())
    }

    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Self::OpenAI => "OpenAI",
            Self::Proxy => "Hosted",
        }
    }

    fn models_url(&self) -> String {
        format!("{}/v1/models", self.base_url())
    }

    /// Available iff this variant can actually be used at runtime. `Proxy` is
    /// only usable when `PROXY_URL` was set at build time.
    pub fn is_available(&self) -> bool {
        match self {
            Self::OpenAI => true,
            Self::Proxy => PROXY_BASE.is_some_and(|url| !url.is_empty()),
        }
    }

    fn is_owned(&self, owned_by: &str) -> bool {
        // OpenAI's `/v1/models` tags its own models under three labels —
        // "openai" (gpt-3.5, legacy gpt-4), "system" (everything newer
        // including gpt-4o, gpt-5, o-series, dall-e-*, gpt-image-*), and
        // "openai-internal" (tts, whisper, embeddings). Anything else is
        // a user-owned fine-tune.
        match self {
            Self::OpenAI | Self::Proxy => {
                matches!(owned_by, "openai" | "system" | "openai-internal")
            }
        }
    }

    fn is_image_id(&self, id: &str) -> bool {
        match self {
            Self::OpenAI | Self::Proxy => {
                id.starts_with("dall-e")
                    || id.starts_with("gpt-image")
                    || id == "chatgpt-image-latest"
            }
        }
    }

    fn is_chat_id(&self, id: &str) -> bool {
        if self.is_image_id(id) {
            return false;
        }
        match self {
            Self::OpenAI | Self::Proxy => {
                (id.starts_with("gpt-") || id.starts_with("o"))
                    && !id.contains("realtime")
                    && !id.contains("audio")
                    && !id.contains("search")
                    && !id.contains("instruct")
                    && !id.contains("transcribe")
                    && !id.contains("tts")
                    && !id.contains("moderation")
            }
        }
    }
}

// --- HTTP helpers ---

/// Resolve the bearer token for a request. OpenAI provider uses the stored
/// API key; Proxy provider fetches a short-lived Firebase ID token.
async fn auth_token(settings: &AiSettings) -> Result<String, AiError> {
    match settings.provider {
        AiProvider::OpenAI => Ok(settings.api_key.clone()),
        AiProvider::Proxy => firebase::get_id_token()
            .await?
            .filter(|token| !token.is_empty())
            .ok_or(AiError::AuthRequired),
    }
}

/// Send an authenticated API request: serialize body, fetch, deserialize
/// response.
async fn api_request<T: Serialize, R: DeserializeOwned>(
    settings: &AiSettings,
    url: &str,
    request: &T,
) -> Result<R, AiError> {
    let body_str = serde_json::to_string(request)?;
    let resp = api_fetch(settings, url, "POST", Some(&body_str)).await?;
    let text = response_text(resp).await?;
    Ok(serde_json::from_str(&text)?)
}

/// Low-level authenticated fetch returning raw `web_sys::Response`.
/// Used by `api_request` and streaming endpoints.
async fn api_fetch(
    settings: &AiSettings,
    url: &str,
    method: &str,
    body: Option<&str>,
) -> Result<web_sys::Response, AiError> {
    let token = auth_token(settings).await?;

    let opts = web_sys::RequestInit::new();
    opts.set_method(method);

    let headers = web_sys::Headers::new()?;
    headers.set("Authorization", &format!("Bearer {token}"))?;
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
        return Err(AiError::Http {
            status,
            body: extract_error_message(&body),
        });
    }

    Ok(resp)
}

#[derive(Deserialize)]
struct ApiErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    message: String,
}

/// OpenAI errors arrive as `{"error":{"message":"..."}}`. Pull just the
/// human-readable message out so the UI can show it on one line; fall back
/// to the raw body if parsing fails. Cuts everything after the first colon
/// — OpenAI tucks the leaked API key, hint URLs and other noise there.
fn extract_error_message(body: &str) -> String {
    let message = serde_json::from_str::<ApiErrorEnvelope>(body)
        .map(|env| env.error.message)
        .unwrap_or_else(|_| body.to_string());
    message
        .split(':')
        .next()
        .unwrap_or(&message)
        .trim()
        .to_string()
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

#[derive(Debug, Default, Clone)]
pub struct ModelLists {
    pub chat: Vec<String>,
    pub image: Vec<String>,
}

/// Fetch all available models and split them into chat and image lists.
/// Single HTTP request, filtered client-side per kind.
pub async fn fetch_models(settings: &AiSettings) -> Result<ModelLists, AiError> {
    let resp = api_fetch(settings, &settings.provider.models_url(), "GET", None).await?;
    let text = response_text(resp).await?;
    let parsed: ModelsResponse = serde_json::from_str(&text)?;

    let provider = settings.provider;
    let (mut chat, mut image): (Vec<String>, Vec<String>) = parsed
        .data
        .into_iter()
        .filter_map(|entry| {
            (provider.is_owned(&entry.owned_by)
                && (provider.is_chat_id(&entry.id) || provider.is_image_id(&entry.id)))
            .then_some(entry.id)
        })
        .partition(|id| provider.is_chat_id(id));
    chat.sort();
    image.sort();
    Ok(ModelLists { chat, image })
}

// --- Settings ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct AiSettings {
    pub provider: AiProvider,
    pub api_key: String,
    pub model: String,
    #[serde(default = "default_image_model")]
    pub image_model: String,
}

fn default_image_model() -> String {
    AiProvider::default().default_image_model().to_string()
}

impl Default for AiSettings {
    fn default() -> Self {
        let provider = AiProvider::default();
        Self {
            model: provider.default_model().to_string(),
            image_model: provider.default_image_model().to_string(),
            api_key: String::new(),
            provider,
        }
    }
}

impl AiSettings {
    /// Returns `true` when the current settings can authenticate a request.
    /// For OpenAI this means a non-empty stored key; for Proxy it means the
    /// user is currently signed in with Google.
    pub fn has_api_key(&self) -> bool {
        match self.provider {
            AiProvider::OpenAI => !self.api_key.trim().is_empty(),
            AiProvider::Proxy => firebase::current_provider().as_deref() == Some("google.com"),
        }
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
    pub class: String,
    pub subclass: String,
    pub background: String,
    pub history: String,
    pub personality_traits: String,
    pub ideals: String,
    pub bonds: String,
    pub flaws: String,
}

impl CharacterContext {
    pub fn from_character(character: &Character) -> Self {
        let class = character
            .identity
            .classes
            .iter()
            .filter(|c| !c.class.is_empty())
            .map(|c| c.class_label().to_string())
            .collect::<Vec<_>>()
            .join(" / ");
        let subclass = character
            .identity
            .classes
            .iter()
            .filter_map(|c| c.subclass_label().map(String::from))
            .collect::<Vec<_>>()
            .join(" / ");
        Self {
            name: character.identity.name.clone(),
            species: character.identity.species.clone(),
            class,
            subclass,
            background: character.identity.background.clone(),
            history: character.personality.history.clone(),
            personality_traits: character.personality.personality_traits.clone(),
            ideals: character.personality.ideals.clone(),
            bonds: character.personality.bonds.clone(),
            flaws: character.personality.flaws.clone(),
        }
    }

    /// Canonical labelled character description, used by every AI prompt.
    /// Inline single-paragraph form (`Name: X. Race: Y. Class: Z. …`) —
    /// the previous multi-line `Label:` column made image models render
    /// the prompt itself as a character sheet template. Empty fields are
    /// skipped. Backstory comes last because it's typically the longest.
    pub fn to_prompt_text(&self) -> String {
        let fields: [(&str, &str); 10] = [
            ("Name", &self.name),
            ("Race", &self.species),
            ("Class", &self.class),
            ("Subclass", &self.subclass),
            ("Background", &self.background),
            ("Personality", &self.personality_traits),
            ("Ideals", &self.ideals),
            ("Bonds", &self.bonds),
            ("Flaws", &self.flaws),
            ("Backstory", &self.history),
        ];
        fields
            .into_iter()
            .filter_map(|(label, value)| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| format!("{label}: {trimmed}"))
            })
            .collect::<Vec<_>>()
            .join(". ")
    }
}

// --- Story generation (streaming) ---

#[derive(Serialize)]
struct StreamingChatRequest<'a> {
    model: &'a str,
    stream: bool,
    messages: Vec<&'a ChatMessage>,
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
    let system_msg = ChatMessage {
        role: Role::System,
        content: STORY_SYSTEM_PROMPT.to_string(),
    };
    let user_msg = ChatMessage {
        role: Role::User,
        content: user_content,
    };
    let request_body = StreamingChatRequest {
        model: &settings.model,
        stream: true,
        messages: vec![&system_msg, &user_msg],
    };
    let body_str = serde_json::to_string(&request_body)?;

    let resp = api_fetch(
        settings,
        &settings.provider.api_url(),
        "POST",
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

        // Process complete SSE lines from the buffer. Advance a cursor
        // instead of draining line-by-line — a single trailing drain keeps
        // the inner loop O(n) per chunk instead of O(n²).
        let mut consumed = 0usize;
        let mut stop = false;
        while let Some(rel_pos) = buffer[consumed..].find('\n') {
            let line_end = consumed + rel_pos;
            let trimmed = buffer[consumed..line_end].trim();

            if let Some(data) = trimmed.strip_prefix("data: ") {
                if data == "[DONE]" {
                    consumed = line_end + 1;
                    stop = true;
                    break;
                }
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data)
                    && let Some(content) = parsed["choices"][0]["delta"]["content"].as_str()
                {
                    full_text.push_str(content);
                    on_chunk(content);
                }
            }

            consumed = line_end + 1;
        }
        buffer.drain(..consumed);

        if stop {
            break;
        }
    }

    Ok(full_text)
}

// --- Non-streaming chat completion ---

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Clone, Serialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
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

fn parse_json_content<T: DeserializeOwned>(content: &str) -> Result<T, AiError> {
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

/// Send a chat request with the given messages and return (parsed JSON,
/// raw content string). The raw content is useful for multi-turn
/// conversations where it needs to be added back as an assistant message.
pub async fn send_chat<T: DeserializeOwned>(
    settings: &AiSettings,
    messages: &[ChatMessage],
) -> Result<(T, String), AiError> {
    let request_body = ChatRequest {
        model: &settings.model,
        messages,
    };

    log::debug!(
        "AI request to {} ({} messages)",
        settings.model,
        messages.len()
    );
    for message in messages {
        log::debug!("[{:?}] {}", message.role, message.content);
    }

    let response: ChatResponse =
        api_request(settings, &settings.provider.api_url(), &request_body).await?;

    let content = response
        .choices
        .into_iter()
        .next()
        .ok_or(AiError::EmptyResponse)?
        .message
        .content;

    log::debug!("AI response:\n{content}");

    let parsed = parse_json_content(&content)?;
    Ok((parsed, content))
}

// --- Image generation ---

/// Smallest canvas each model supports. Our downstream pipeline crops to
/// 3:4 and scales to 384×512, so any oversampling past 1024 is waste.
/// dall-e-2 is the only family that can go below 1024 per side.
pub fn image_size_for(model: &str) -> &'static str {
    if model.starts_with("dall-e-2") {
        "512x512"
    } else {
        "1024x1024"
    }
}

#[derive(Serialize)]
struct ImageRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    n: u8,
    size: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<&'a str>,
}

/// `gpt-image-*` family doesn't accept `response_format` and always
/// returns base64. Including the parameter 400s the request.
fn response_format_for(model: &str) -> Option<&'static str> {
    if model.starts_with("gpt-image") || model == "chatgpt-image-latest" {
        None
    } else {
        Some("b64_json")
    }
}

#[derive(Deserialize)]
struct ImageResponse {
    data: Vec<ImageItem>,
}

#[derive(Deserialize)]
struct ImageItem {
    b64_json: String,
}

/// Generate a portrait via the provider's image API. Returns the raw base64
/// PNG bytes (no `data:` prefix).
pub async fn generate_avatar_image(settings: &AiSettings, prompt: &str) -> Result<String, AiError> {
    let request = ImageRequest {
        model: &settings.image_model,
        prompt,
        n: 1,
        size: image_size_for(&settings.image_model),
        response_format: response_format_for(&settings.image_model),
    };
    let response: ImageResponse =
        api_request(settings, &settings.provider.images_url(), &request).await?;
    response
        .data
        .into_iter()
        .next()
        .map(|item| item.b64_json)
        .ok_or(AiError::EmptyResponse)
}

/// Build an English portrait prompt. Kept deliberately minimal: extra
/// stylistic guidance ("painterly portrait painting") and anti-sheet
/// negatives ("NOT a character sheet, NOT infographic") were primes that
/// pushed the model toward rendering character-sheet templates rather
/// than away from them. The opener mirrors what the user found worked:
/// "what kind of artifact" + canvas size + composition, then the shared
/// labelled character description.
pub fn build_avatar_prompt(context: &CharacterContext, user_addition: &str, size: &str) -> String {
    let mut out = String::with_capacity(512);
    let _ = write!(
        out,
        "DnD character avatar. {size} canvas, vertical 3:4 portrait, head-and-shoulders framing. \
         No text in the image.\n\n",
    );
    out.push_str(&context.to_prompt_text());
    let extra = user_addition.trim();
    if !extra.is_empty() {
        let _ = write!(out, "\nExtra details: {extra}");
    }
    out
}

/// Non-streaming chat completion that parses the response content as JSON of
/// type `T`. Convenience wrapper around `send_chat` for single-turn requests.
async fn chat_completion<T: DeserializeOwned>(
    settings: &AiSettings,
    system_prompt: &str,
    user_message: &str,
) -> Result<T, AiError> {
    let messages = vec![
        ChatMessage {
            role: Role::System,
            content: system_prompt.to_string(),
        },
        ChatMessage {
            role: Role::User,
            content: user_message.to_string(),
        },
    ];
    let (result, _) = send_chat(settings, &messages).await?;
    Ok(result)
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
         Respond with a JSON object with these fields:\n\
         - name: string\n\
         - species: string (exactly one of the available species)\n\
         - class: string (exactly one of the available classes)\n\
         - subclass: string or null (exactly one of the listed subclasses)\n\
         - background: string (exactly one of the available backgrounds)\n\
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

pub struct PendingReplacementDescription {
    pub feature_name: String,
    pub feature_description: String,
    /// (name, description) pairs for eligible replacement features.
    pub eligible: Vec<(String, String)>,
}

/// Build AI-readable descriptions of replaceable features (features with
/// `replace_with`). For each, lists eligible replacement options.
pub fn describe_pending_replacements(
    pending: &[PendingInputs],
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
    character: &Character,
) -> Vec<PendingReplacementDescription> {
    // Pre-filter: evaluate prerequisites once per feature, not per replaceable
    // input
    let selectable: Vec<_> = features_index
        .values()
        .filter(|feat| feat.is_selectable() && feat.meets_prerequisites(character))
        .collect();

    pending
        .iter()
        .filter(|input| input.is_replaceable())
        .filter_map(|input| {
            let eligible: Vec<(String, String)> = selectable
                .iter()
                .filter(|feat| input.replace_with.matches(feat))
                .map(|feat| (feat.name.to_string(), feat.description.clone()))
                .collect();

            if eligible.is_empty() {
                return None;
            }

            Some(PendingReplacementDescription {
                feature_name: input.feature_name.clone(),
                feature_description: input.feature_description.clone(),
                eligible,
            })
        })
        .collect()
}

// --- ArgSummarizer: Interpreter that extracts ARG descriptions ---

/// Map an Attribute to a human-readable English name for AI prompts.
fn friendly_attr_name(attr: &Attribute) -> &'static str {
    use Ability::*;
    use Skill::*;
    match attr {
        Attribute::Ability(Strength) => "Strength",
        Attribute::Ability(Dexterity) => "Dexterity",
        Attribute::Ability(Constitution) => "Constitution",
        Attribute::Ability(Intelligence) => "Intelligence",
        Attribute::Ability(Wisdom) => "Wisdom",
        Attribute::Ability(Charisma) => "Charisma",
        Attribute::SkillProficiency(Acrobatics) => "Acrobatics proficiency",
        Attribute::SkillProficiency(AnimalHandling) => "Animal Handling proficiency",
        Attribute::SkillProficiency(Arcana) => "Arcana proficiency",
        Attribute::SkillProficiency(Athletics) => "Athletics proficiency",
        Attribute::SkillProficiency(Deception) => "Deception proficiency",
        Attribute::SkillProficiency(History) => "History proficiency",
        Attribute::SkillProficiency(Insight) => "Insight proficiency",
        Attribute::SkillProficiency(Intimidation) => "Intimidation proficiency",
        Attribute::SkillProficiency(Investigation) => "Investigation proficiency",
        Attribute::SkillProficiency(Medicine) => "Medicine proficiency",
        Attribute::SkillProficiency(Nature) => "Nature proficiency",
        Attribute::SkillProficiency(Perception) => "Perception proficiency",
        Attribute::SkillProficiency(Performance) => "Performance proficiency",
        Attribute::SkillProficiency(Persuasion) => "Persuasion proficiency",
        Attribute::SkillProficiency(Religion) => "Religion proficiency",
        Attribute::SkillProficiency(SleightOfHand) => "Sleight of Hand proficiency",
        Attribute::SkillProficiency(Stealth) => "Stealth proficiency",
        Attribute::SkillProficiency(Survival) => "Survival proficiency",
        Attribute::Ac => "Armor Class",
        Attribute::Speed => "Speed",
        Attribute::MaxHp => "Max HP",
        _ => "unknown",
    }
}

/// Info about a single ARG extracted by [`ArgSummarizer`].
struct ArgInfo {
    target: Option<&'static str>,
    range: Option<(i32, i32)>,
}

/// Result produced by [`ArgSummarizer`].
struct ArgSummary {
    args: BTreeMap<u8, ArgInfo>,
    sum_constraint: Option<i32>,
    /// For loop expressions: target group members.
    group_members: Vec<Attribute>,
}

/// Stack entry for the summarizer: tracks whether a value came from an ARG.
struct ArgStackEntry {
    /// `Some(idx)` if this value is (or derives from) a single ARG.
    arg_idx: Option<u8>,
    /// Numeric value if known at analysis time.
    num: Option<i32>,
    /// Number of ARG values summed (for sum constraint detection).
    sum_count: u32,
}

impl ArgStackEntry {
    fn constant(n: i32) -> Self {
        Self {
            arg_idx: None,
            num: Some(n),
            sum_count: 0,
        }
    }

    fn arg(idx: u8) -> Self {
        Self {
            arg_idx: Some(idx),
            num: None,
            sum_count: 1,
        }
    }

    fn other() -> Self {
        Self {
            arg_idx: None,
            num: None,
            sum_count: 0,
        }
    }
}

struct ArgSummarizer {
    stack: Vec<ArgStackEntry>,
    iter_stack: IterStack,
    args: BTreeMap<u8, ArgInfo>,
    sum_constraint: Option<i32>,
    group_members: Vec<Attribute>,
}

impl ArgSummarizer {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            iter_stack: IterStack::new(),
            args: BTreeMap::new(),
            sum_constraint: None,
            group_members: Vec::new(),
        }
    }

    fn pop(&mut self) -> ArgStackEntry {
        self.stack.pop().unwrap_or(ArgStackEntry::constant(0))
    }

    fn ensure_arg(&mut self, idx: u8) -> &mut ArgInfo {
        self.args.entry(idx).or_insert(ArgInfo {
            target: None,
            range: None,
        })
    }

    fn binary_op(&mut self, bin_op: BinOp) {
        let b = self.pop();
        let a = self.pop();
        // Propagate arg_idx through binary ops (e.g. SKILL.PROF += ARG.0
        // compiles to PushVar(SKILL) PushVar(ARG.0) Add Assign(SKILL))
        let arg_idx = a.arg_idx.or(b.arg_idx);
        // Only track sum_count and numeric value for Add (sum constraint detection)
        let (num, sum_count) = if matches!(bin_op, BinOp::Add) {
            (
                a.num.zip(b.num).map(|(a, b)| a + b),
                a.sum_count + b.sum_count,
            )
        } else {
            (None, 0)
        };
        self.stack.push(ArgStackEntry {
            arg_idx,
            num,
            sum_count,
        });
    }
}

impl Interpreter<Attribute, i32, AttributeGroup> for ArgSummarizer {
    type Output = ArgSummary;

    fn exec(&mut self, op: Op) -> Result<Option<BlockIndex>, expr::Error> {
        match op {
            Op::PushVar(Attribute::Arg(idx)) => {
                self.ensure_arg(idx);
                self.stack.push(ArgStackEntry::arg(idx));
            }
            Op::PushVar(_) => {
                self.stack.push(ArgStackEntry::other());
            }
            Op::PushConst(n) => {
                self.stack.push(ArgStackEntry::constant(n));
            }
            Op::AssignVar(attr) => {
                let value = self.pop();
                if let Some(idx) = value.arg_idx {
                    let info = self.ensure_arg(idx);
                    if info.target.is_none() {
                        info.target = Some(friendly_attr_name(&attr));
                    }
                }
            }
            Op::In => {
                // in(value, min, max): value is on stack first, then min, then max
                // But RPN order: push value, push min, push max, In
                // Stack: [..., value, min, max] → pop3 → (value, min, max)
                let max_entry = self.pop();
                let min_entry = self.pop();
                let value_entry = self.pop();
                if let (Some(idx), Some(min), Some(max)) =
                    (value_entry.arg_idx, min_entry.num, max_entry.num)
                {
                    self.ensure_arg(idx).range = Some((min, max));
                }
                self.stack.push(ArgStackEntry::other());
            }
            Op::BinOp(bin_op) => self.binary_op(bin_op),
            Op::Cmp(Cmp::Eq) => {
                let b = self.pop();
                let a = self.pop();
                // Detect "sum_of_args == N" pattern
                if a.sum_count > 1 && b.num.is_some() {
                    self.sum_constraint = b.num;
                } else if b.sum_count > 1 && a.num.is_some() {
                    self.sum_constraint = a.num;
                }
                self.stack.push(ArgStackEntry::other());
            }
            Op::Cmp(_) => {
                self.pop();
                self.pop();
                self.stack.push(ArgStackEntry::other());
            }
            // Recurse into sub-blocks (guard body, if branches).
            // Skip when condition is known constant(0) — loop termination from Next.
            Op::EvalIf(then_idx, _else_idx) => {
                let cond = self.pop();
                if cond.num != Some(0)
                    && then_idx != expr::BLOCK_NOOP
                    && then_idx != expr::BLOCK_ERROR
                {
                    return Ok(Some(then_idx));
                }
            }
            Op::Eval(idx) => {
                if idx != expr::BLOCK_NOOP {
                    return Ok(Some(idx));
                }
            }
            Op::Not | Op::AvgHp => {
                self.pop();
                self.stack.push(ArgStackEntry::other());
            }
            Op::Roll | Op::Sum | Op::Explode => {
                self.pop();
                self.pop();
                self.stack.push(ArgStackEntry::other());
            }
            Op::KeepMax(_) | Op::KeepMin(_) | Op::DropMax(_) | Op::DropMin(_) => {
                let top = self.pop();
                self.stack.push(top);
            }
            Op::Each(subgrp) => {
                if self.group_members.is_empty() {
                    self.group_members.extend(subgrp.members());
                }
                let has_items = subgrp.init_loop(&mut self.iter_stack);
                self.stack.push(ArgStackEntry::constant(has_items as i32));
            }
            Op::Next(subgrp) => {
                let more = subgrp.advance_loop(&mut self.iter_stack);
                self.stack.push(ArgStackEntry::constant(more as i32));
            }
            Op::PushGroup(_) => {
                self.stack.push(ArgStackEntry::other());
            }
            Op::AssignGroup(_) => {
                self.pop();
            }
            Op::Tier(count) => {
                self.pop(); // var
                for _ in 0..count {
                    self.pop(); // value
                    self.pop(); // threshold
                }
                self.stack.push(ArgStackEntry::other());
            }
        }
        Ok(None)
    }

    fn finish(self) -> Result<Self::Output, expr::Error> {
        Ok(ArgSummary {
            args: self.args,
            sum_constraint: self.sum_constraint,
            group_members: self.group_members,
        })
    }
}

/// Convert pending feature inputs into AI-readable descriptions of their ARG
/// parameters. Features with no active ARGs are skipped.
pub fn describe_pending_args(
    pending: &[PendingInputs],
    character: &Character,
) -> Vec<PendingArgDescription> {
    pending
        .iter()
        .filter_map(|inputs| {
            let mut description = String::new();
            let mut had_args = false;

            for expr in &inputs.exprs {
                let analysis = expr.analyze(character, Attribute::arg_index);
                if analysis.active_args.is_empty() {
                    continue;
                }
                had_args = true;

                // Run ArgSummarizer for non-loop expressions (extracts per-ARG
                // targets and ranges). Loop expressions use ExprAnalysis fields.
                let summary = expr.run(ArgSummarizer::new()).unwrap_or(ArgSummary {
                    args: BTreeMap::new(),
                    sum_constraint: None,
                    group_members: Vec::new(),
                });

                let sum_constraint = summary.sum_constraint;

                let all_boolean = analysis
                    .active_args
                    .iter()
                    .all(|idx| analysis.boolean_args.contains(idx));
                if let Some(sum) = sum_constraint {
                    if all_boolean {
                        let _ = writeln!(
                            description,
                            "Pick exactly {sum} (set chosen to 1, rest to 0):"
                        );
                    } else {
                        let _ = writeln!(description, "Distribute exactly {sum} points total:");
                    }
                }

                // Per-ARG description: use group names from ExprAnalysis for
                // loop expressions, ArgSummarizer info for flat expressions.
                for (pos, &arg_index) in analysis.active_args.iter().enumerate() {
                    // Target name: group member name (loop) or ArgSummarizer target (flat)
                    // group_names uses virtual positions (0, 1, 2…) while active_args
                    // may contain real group indices (0, 3, 5…) for masked subgroups,
                    // so index group_names by position within active_args.
                    let group_name = summary
                        .group_members
                        .get(pos)
                        .map(|attr| friendly_attr_name(attr));
                    let summarizer_name = summary.args.get(&arg_index).and_then(|info| info.target);
                    let target = group_name.or(summarizer_name).unwrap_or("unknown");

                    // Range: from ArgSummarizer (flat) or boolean from ExprAnalysis
                    let range = summary.args.get(&arg_index).and_then(|info| info.range);

                    if let Some((min, max)) = range {
                        if min == 0 && max == 1 {
                            let _ = writeln!(description, "  ARG.{arg_index}: {target} — 0 or 1");
                        } else {
                            let _ = writeln!(
                                description,
                                "  ARG.{arg_index}: {target} — integer in [{min}, {max}]"
                            );
                        }
                    } else if analysis.boolean_args.contains(&arg_index) {
                        let _ = writeln!(description, "  ARG.{arg_index}: {target} — 0 or 1");
                    } else {
                        let _ = writeln!(description, "  ARG.{arg_index}: {target} — integer");
                    }
                }
            }

            if !had_args {
                return None;
            }

            Some(PendingArgDescription {
                feature_name: inputs.feature_name.clone(),
                feature_description: inputs.feature_description.clone(),
                args_description: description,
            })
        })
        .collect()
}

/// Parse AI response that may contain both ARG values and replacements.
/// Returns `(args, replacements)`.
pub fn parse_feature_choices_response(
    value: serde_json::Value,
) -> (BTreeMap<String, Vec<i32>>, BTreeMap<String, String>) {
    let Some(obj) = value.as_object() else {
        return (BTreeMap::new(), BTreeMap::new());
    };

    let replacements: BTreeMap<String, String> = obj
        .get("replacements")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let args: BTreeMap<String, Vec<i32>> = obj
        .iter()
        .filter(|(key, _)| *key != "replacements")
        .filter_map(|(key, value)| {
            let arr: Vec<i32> = serde_json::from_value(value.clone()).ok()?;
            // Normalize: strip "Feature: " prefix that some models add
            let key = key.strip_prefix("Feature: ").unwrap_or(key).to_string();
            Some((key, arr))
        })
        .collect();

    (args, replacements)
}

/// Build the initial messages for the feature choices conversation.
/// Returns `[system, user]` messages ready for `send_chat`.
pub fn build_feature_choices_messages(
    concept: &CharacterConcept,
    pending_args: &[PendingArgDescription],
    pending_replacements: &[PendingReplacementDescription],
) -> Vec<ChatMessage> {
    let has_replacements = !pending_replacements.is_empty();

    let mut system_prompt = String::from(
        "You are a D&D 5e character builder. Given a character concept and \
        pending feature choices, pick ARG values that best fit the character. \
        Respond with valid JSON only, no markdown.\n\n\
        Each feature lists its ARGs with names and constraints. \
        Boolean ARGs (0 or 1) are toggles — set to 1 to select, 0 to skip. \
        When the description says \"Pick exactly N\", exactly N ARGs must be 1. \
        When it says \"Distribute N points\", the ARG values must sum to N. \
        Each ARG's valid range is shown after the dash.",
    );

    if has_replacements {
        system_prompt.push_str(
            "\n\nFor replaceable features, choose the best fitting replacement \
            from the eligible list. Add a \"replacements\" field to your response: \
            { \"Feature Name\": [ARGs], \"replacements\": { \"Original\": \"Replacement\" } }",
        );
    } else {
        system_prompt.push_str(
            "\n\nRespond with a JSON object: { \"Feature Name\": [ARG.0, ARG.1, ...], ... }",
        );
    }

    let features_description: String = join_iter(
        pending_args.iter().map(|pending| {
            format!(
                "- \"{}\"\n  Description: {}\n{}",
                pending.feature_name, pending.feature_description, pending.args_description
            )
        }),
        "\n\n",
    );

    let replacements_description: String = join_iter(
        pending_replacements.iter().map(|pending| {
            let options: String = join_iter(
                pending.eligible.iter().map(|(name, desc)| {
                    let short_desc = if desc.len() > 80 { &desc[..80] } else { desc };
                    format!("    - \"{name}\" — {short_desc}")
                }),
                "\n",
            );
            format!(
                "- \"{}\" — {}\n  Choose one replacement:\n{}",
                pending.feature_name, pending.feature_description, options
            )
        }),
        "\n\n",
    );

    let mut user_message = format!(
        "Character concept:\n\
         Name: {name}\n\
         Species: {species}\n\
         Class: {class}\n\
         Background: {background}\n\
         Personality: {personality}\n\
         Backstory: {backstory}",
        name = concept.name,
        species = concept.species,
        class = concept.class,
        background = concept.background,
        personality = concept.personality_traits,
        backstory = concept.backstory,
    );

    if !features_description.is_empty() {
        let _ = write!(
            user_message,
            "\n\nPending feature choices:\n{features_description}"
        );
    }
    if !replacements_description.is_empty() {
        let _ = write!(
            user_message,
            "\n\nReplaceable features (pick one replacement for each):\n{replacements_description}"
        );
    }
    user_message.push_str(
        "\n\nRespond with a JSON object where each key is the exact feature name \
         and each value is an array of integer ARG values.",
    );
    if has_replacements {
        user_message.push_str(
            " Include a \"replacements\" field mapping each original feature name \
             to the exact replacement name from the eligible list.",
        );
    }

    vec![
        ChatMessage {
            role: Role::System,
            content: system_prompt,
        },
        ChatMessage {
            role: Role::User,
            content: user_message,
        },
    ]
}
