use std::{collections::BTreeMap, fmt::Write};

use js_sys::{Date, Object, Reflect};
use reactive_stores::Store;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::{
    expr::{self, BinOp, BlockIndex, Cmp, Interpreter, VarGroup},
    model::{Attribute, AttributeGroup, Character, Op},
    rules::{FeatureDefinition, PendingInputs},
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

    let parsed = parse_json_content(&content)?;
    Ok((parsed, content))
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
    pending
        .iter()
        .filter(|input| input.is_replaceable())
        .filter_map(|input| {
            let eligible: Vec<(String, String)> = features_index
                .values()
                .filter(|feat| {
                    input.replace_with.matches(feat) && feat.meets_prerequisites(character)
                })
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

/// Map an Attribute Display string to a human-readable English name for AI
/// prompts. Accepts the `Display` representation (e.g. "STR",
/// "SKILL.ACRO.PROF").
fn friendly_attr_name(attr_str: &str) -> &str {
    match attr_str {
        "STR" => "Strength",
        "DEX" => "Dexterity",
        "CON" => "Constitution",
        "INT" => "Intelligence",
        "WIS" => "Wisdom",
        "CHA" => "Charisma",
        "SKILL.ACRO.PROF" => "Acrobatics proficiency",
        "SKILL.ANIM.PROF" => "Animal Handling proficiency",
        "SKILL.ARCA.PROF" => "Arcana proficiency",
        "SKILL.ATHL.PROF" => "Athletics proficiency",
        "SKILL.DECE.PROF" => "Deception proficiency",
        "SKILL.HIST.PROF" => "History proficiency",
        "SKILL.INSI.PROF" => "Insight proficiency",
        "SKILL.INTI.PROF" => "Intimidation proficiency",
        "SKILL.INVE.PROF" => "Investigation proficiency",
        "SKILL.MEDI.PROF" => "Medicine proficiency",
        "SKILL.NATU.PROF" => "Nature proficiency",
        "SKILL.PERC.PROF" => "Perception proficiency",
        "SKILL.PERF.PROF" => "Performance proficiency",
        "SKILL.PERS.PROF" => "Persuasion proficiency",
        "SKILL.RELI.PROF" => "Religion proficiency",
        "SKILL.SLEI.PROF" => "Sleight of Hand proficiency",
        "SKILL.STEA.PROF" => "Stealth proficiency",
        "SKILL.SURV.PROF" => "Survival proficiency",
        "AC" => "Armor Class",
        "SPEED" => "Speed",
        "MAX_HP" => "Max HP",
        other => other,
    }
}

/// Info about a single ARG extracted by [`ArgSummarizer`].
struct ArgInfo {
    target: Option<String>,
    range: Option<(i32, i32)>,
}

/// Result produced by [`ArgSummarizer`].
struct ArgSummary {
    args: BTreeMap<u8, ArgInfo>,
    sum_constraint: Option<i32>,
    /// For loop expressions: Display names of target group members.
    group_names: Vec<String>,
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
    iter_stack: Vec<usize>,
    args: BTreeMap<u8, ArgInfo>,
    sum_constraint: Option<i32>,
    group_names: Vec<String>,
}

impl ArgSummarizer {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            iter_stack: Vec::new(),
            args: BTreeMap::new(),
            sum_constraint: None,
            group_names: Vec::new(),
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
                        info.target = Some(friendly_attr_name(&attr.to_string()).to_string());
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
                // Collect group member names for AI descriptions
                if self.group_names.is_empty() {
                    self.group_names.extend(
                        (0..).map_while(|idx| subgrp.member(idx).map(|var| var.to_string())),
                    );
                }
                self.iter_stack.push(subgrp.real_index(0).unwrap_or(0));
                self.stack.push(ArgStackEntry::constant(
                    subgrp
                        .inner
                        .member(subgrp.real_index(0).unwrap_or(0))
                        .is_some() as i32,
                ));
            }
            Op::Next(subgrp) => {
                if let Some(&current) = self.iter_stack.last()
                    && let Some(next_idx) = subgrp.next_real_index(current)
                    && subgrp.inner.member(next_idx).is_some()
                {
                    *self.iter_stack.last_mut().unwrap() = next_idx;
                    self.stack.push(ArgStackEntry::constant(1));
                } else {
                    self.iter_stack.pop();
                    self.stack.push(ArgStackEntry::constant(0));
                }
            }
            Op::PushGroup(_) => {
                self.stack.push(ArgStackEntry::other());
            }
            Op::AssignGroup(_) => {
                self.pop();
            }
        }
        Ok(None)
    }

    fn finish(self) -> Result<Self::Output, expr::Error> {
        Ok(ArgSummary {
            args: self.args,
            sum_constraint: self.sum_constraint,
            group_names: self.group_names,
        })
    }
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
            let mut description = String::new();
            let mut had_args = false;

            for expr in &inputs.exprs {
                let analysis = expr.analyze(character, is_arg);
                if analysis.active_args.is_empty() {
                    continue;
                }
                had_args = true;

                // Run ArgSummarizer for non-loop expressions (extracts per-ARG
                // targets and ranges). Loop expressions use ExprAnalysis fields.
                let summary = expr.run(ArgSummarizer::new()).unwrap_or(ArgSummary {
                    args: BTreeMap::new(),
                    sum_constraint: None,
                    group_names: Vec::new(),
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
                        .group_names
                        .get(pos)
                        .map(|name| friendly_attr_name(name).to_string());
                    let summarizer_name = summary
                        .args
                        .get(&arg_index)
                        .and_then(|info| info.target.clone());
                    let target = group_name
                        .or(summarizer_name)
                        .unwrap_or_else(|| "unknown".to_string());

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
            Some((key.clone(), arr))
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

    let features_description: String = pending_args
        .iter()
        .map(|pending| {
            format!(
                "- \"{}\"\n  Description: {}\n{}",
                pending.feature_name, pending.feature_description, pending.args_description
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let replacements_description: String = pending_replacements
        .iter()
        .map(|pending| {
            let options: String = pending
                .eligible
                .iter()
                .map(|(name, desc)| {
                    let short_desc = if desc.len() > 80 { &desc[..80] } else { desc };
                    format!("    - \"{name}\" — {short_desc}")
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "- \"{}\" — {}\n  Choose one replacement:\n{}",
                pending.feature_name, pending.feature_description, options
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

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
