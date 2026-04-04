# AI Story Generator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an AI-powered inter-session story generator with OpenAI streaming, accessible via `/c/:id/story` with a two-column reference-style layout.

**Architecture:** New `src/ai.rs` module handles OpenAI API streaming via `gloo_net` + `web_sys::ReadableStreamDefaultReader`. Story data stored in localStorage separately from Character (like effects). New page component with reference-layout sidebar for story navigation.

**Tech Stack:** Leptos 0.8 CSR, gloo-net (HTTP), web-sys (ReadableStream SSE parsing), gloo-storage (localStorage), serde_json (API payloads)

---

### Task 1: Data model and storage

**Files:**
- Create: `src/ai.rs`
- Modify: `src/storage.rs`
- Modify: `src/lib.rs` (add `mod ai;`)

- [ ] **Step 1: Create `src/ai.rs` with data types**

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let mut parts = vec![
            format!(
                "Character: {}, Level {} {} {}",
                self.name, self.level, self.species, self.class_summary
            ),
        ];
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
```

- [ ] **Step 2: Add `mod ai;` to `src/lib.rs`**

In `src/lib.rs`, add `mod ai;` to the module declarations (after `mod demap;`):

```rust
mod ai;
```

- [ ] **Step 3: Add storage functions to `src/storage.rs`**

Add at the end of `src/storage.rs`:

```rust
use crate::ai::{AiSettings, Story};

const AI_SETTINGS_KEY: &str = "dnd_pc_ai_settings";

fn stories_key(id: &Uuid) -> String {
    format!("dnd_pc_stories_{id}")
}

pub fn load_ai_settings() -> AiSettings {
    LocalStorage::get(AI_SETTINGS_KEY).unwrap_or_default()
}

pub fn save_ai_settings(settings: &AiSettings) {
    if let Err(error) = LocalStorage::set(AI_SETTINGS_KEY, settings) {
        log::error!("Failed to save AI settings: {error}");
    }
}

pub fn load_stories(id: &Uuid) -> Vec<Story> {
    LocalStorage::get(stories_key(id)).unwrap_or_default()
}

pub fn save_stories(id: &Uuid, stories: &[Story]) {
    if let Err(error) = LocalStorage::set(stories_key(id), stories) {
        log::error!("Failed to save stories: {error}");
    }
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo clippy --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add src/ai.rs src/lib.rs src/storage.rs
git commit -m "feat: add AI story generator data model and storage"
```

---

### Task 2: OpenAI streaming client

**Files:**
- Modify: `src/ai.rs`
- Modify: `Cargo.toml` (add web-sys features)

- [ ] **Step 1: Add required web-sys features to `Cargo.toml`**

Add these features to the `web-sys` dependency in `Cargo.toml`:

```
"ReadableStreamDefaultReader",
"ReadableStreamReadResult",
"Headers",
"RequestInit",
"RequestMode",
"Request",
```

- [ ] **Step 2: Add streaming generate function to `src/ai.rs`**

Add at the end of `src/ai.rs`:

```rust
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

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

    let request = web_sys::Request::new_with_str_and_init(
        settings.provider.api_url(),
        &opts,
    )
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
    let decoder = js_sys::TextDecoder::new().map_err(|error| format!("{error:?}"))?;

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

        let chunk_text = decoder
            .decode_with_buffer_source(&value)
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
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(content) = parsed["choices"][0]["delta"]["content"].as_str() {
                        full_text.push_str(content);
                        on_chunk(content);
                    }
                }
            }
        }
    }

    Ok(full_text)
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo clippy --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src/ai.rs Cargo.toml
git commit -m "feat: add OpenAI streaming client for story generation"
```

---

### Task 3: Routing, i18n keys, and page scaffolding

**Files:**
- Create: `src/pages/character/story.rs`
- Modify: `src/pages/character/mod.rs`
- Modify: `src/lib.rs` (add routes)
- Modify: `src/components/navbar.rs` (add Story link)
- Modify: `src/hooks.rs` (add Story page kind)
- Modify: `locales/en/main.ftl`
- Modify: `locales/ru/main.ftl`

- [ ] **Step 1: Add i18n keys to `locales/en/main.ftl`**

Add near the `view-summary` / `view-full-sheet` keys:

```ftl
view-story = Story
story-new = New Story
story-prompt-placeholder = Describe what happened between sessions...
story-generate = Generate
story-stop = Stop
story-no-api-key = Configure your API key to generate stories.
story-settings = AI Settings
story-api-key = API Key
story-model = Model
story-save = Save
story-delete = Delete
story-copy = Copy
story-copied = Copied!
story-error = Generation error
story-empty = No stories yet. Write a prompt and generate your first story!
story-select = Select a story or create a new one
story-retry = Retry
```

- [ ] **Step 2: Add i18n keys to `locales/ru/main.ftl`**

Add near the `view-summary` / `view-full-sheet` keys:

```ftl
view-story = История
story-new = Новая история
story-prompt-placeholder = Опишите, что произошло между сессиями...
story-generate = Сгенерировать
story-stop = Стоп
story-no-api-key = Настройте API-ключ для генерации историй.
story-settings = Настройки AI
story-api-key = API-ключ
story-model = Модель
story-save = Сохранить
story-delete = Удалить
story-copy = Копировать
story-copied = Скопировано!
story-error = Ошибка генерации
story-empty = Пока нет историй. Напишите промпт и сгенерируйте первую историю!
story-select = Выберите историю или создайте новую
story-retry = Повторить
```

- [ ] **Step 3: Create `src/pages/character/story.rs` with a placeholder**

```rust
use leptos::prelude::*;
use leptos_fluent::move_tr;

#[component]
pub fn CharacterStory() -> impl IntoView {
    view! {
        <div class="reference-page">
            <div class="reference-layout">
                <aside class="reference-sidebar">
                    <p>{move_tr!("story-empty")}</p>
                </aside>
                <main class="reference-main">
                    <p>{move_tr!("story-select")}</p>
                </main>
            </div>
        </div>
    }
}
```

- [ ] **Step 4: Register the module in `src/pages/character/mod.rs`**

Add:

```rust
pub mod story;
```

- [ ] **Step 5: Add routes to `src/lib.rs`**

Add to imports:

```rust
use pages::character::story::CharacterStory;
```

Add nested routes inside the `/c/:id` `ParentRoute`, after the `/quick-start` route:

```rust
<Route path=path!("/story") view=CharacterStory />
<Route path=path!("/story/:story_id") view=CharacterStory />
```

- [ ] **Step 6: Add "Story" link to navbar in `src/components/navbar.rs`**

Inside the `active_id.get().map(|id| ...)` block, after the Summary `<A>` link, add:

```rust
<A href=format!("{BASE_URL}/c/{id}/story") attr:class="navbar-link navbar-link-story">
    {move_tr!("view-story")}
</A>
```

- [ ] **Step 7: Add `Story` variant to `PageKind` in `src/hooks.rs`**

Add `Story` to the `PageKind` enum:

```rust
pub enum PageKind {
    Main,
    Character,
    QuickStart,
    Reference,
    Share,
    Story,
}
```

Add match arm in `as_str()`:

```rust
Self::Story => "story",
```

Add match arm in `use_page_kind()` before the `"c" => PageKind::Character` line:

```rust
"c" if tail.starts_with("story") => PageKind::Story,
```

(Must come before `"c" => PageKind::Character` so it matches first.)

- [ ] **Step 8: Verify it compiles and the page loads**

Run: `cargo clippy --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 9: Commit**

```bash
git add src/pages/character/story.rs src/pages/character/mod.rs src/lib.rs src/components/navbar.rs src/hooks.rs locales/en/main.ftl locales/ru/main.ftl
git commit -m "feat: add story page routing, navbar link, and i18n keys"
```

---

### Task 4: Settings modal component

**Files:**
- Modify: `src/pages/character/story.rs`

- [ ] **Step 1: Add settings modal to story page**

Replace the contents of `src/pages/character/story.rs` with:

```rust
use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{
    ai::AiSettings,
    components::modal::Modal,
    storage,
};

#[component]
fn AiSettingsModal(show: RwSignal<bool>) -> impl IntoView {
    let settings = RwSignal::new(storage::load_ai_settings());

    // Reload settings each time the modal opens
    Effect::new(move || {
        if show.get() {
            settings.set(storage::load_ai_settings());
        }
    });

    let on_save = move |_| {
        storage::save_ai_settings(&settings.get_untracked());
        show.set(false);
    };

    view! {
        <Modal show title=move_tr!("story-settings")>
            <div class="modal-body">
                <div class="textarea-field">
                    <label>{move_tr!("story-api-key")}</label>
                    <input
                        type="password"
                        prop:value=move || settings.get().api_key
                        on:input=move |event| {
                            settings.update(|s| s.api_key = event_target_value(&event));
                        }
                    />
                </div>
                <div class="textarea-field">
                    <label>{move_tr!("story-model")}</label>
                    <input
                        type="text"
                        prop:value=move || settings.get().model
                        on:input=move |event| {
                            settings.update(|s| s.model = event_target_value(&event));
                        }
                    />
                </div>
                <div class="modal-actions">
                    <button on:click=on_save>{move_tr!("story-save")}</button>
                </div>
            </div>
        </Modal>
    }
}

#[component]
pub fn CharacterStory() -> impl IntoView {
    let show_settings = RwSignal::new(false);

    view! {
        <div class="reference-page">
            <div class="reference-layout">
                <aside class="reference-sidebar">
                    <p>{move_tr!("story-empty")}</p>
                </aside>
                <main class="reference-main">
                    <div>
                        <button on:click=move |_| show_settings.set(true)>
                            {move_tr!("story-settings")}
                        </button>
                    </div>
                    <p>{move_tr!("story-select")}</p>
                </main>
            </div>
        </div>
        <AiSettingsModal show=show_settings />
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo clippy --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src/pages/character/story.rs
git commit -m "feat: add AI settings modal to story page"
```

---

### Task 5: Story generation page (new story view)

**Files:**
- Modify: `src/pages/character/story.rs`

This task builds the main generation UI for the `/c/:id/story` route: prompt textarea, Generate/Stop button, streaming output area.

- [ ] **Step 1: Implement the full story page with generation and sidebar**

Replace `src/pages/character/story.rs` with the complete implementation:

```rust
use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use leptos_router::{components::A, hooks::use_params, params::Params};
use reactive_stores::Store;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;

use crate::{
    BASE_URL,
    ai::{AiSettings, CharacterContext, Story, generate_story},
    components::{icon::Icon, modal::Modal},
    model::{Character, CharacterIdentityStoreFields, CharacterStoreFields},
    pages::reference::ReferenceSidebar,
    storage,
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct StoryParams {
    story_id: Option<Uuid>,
}

// --- Settings Modal ---

#[component]
fn AiSettingsModal(show: RwSignal<bool>) -> impl IntoView {
    let settings = RwSignal::new(storage::load_ai_settings());

    Effect::new(move || {
        if show.get() {
            settings.set(storage::load_ai_settings());
        }
    });

    let on_save = move |_| {
        storage::save_ai_settings(&settings.get_untracked());
        show.set(false);
    };

    view! {
        <Modal show title=move_tr!("story-settings")>
            <div class="modal-body">
                <div class="textarea-field">
                    <label>{move_tr!("story-api-key")}</label>
                    <input
                        type="password"
                        prop:value=move || settings.get().api_key
                        on:input=move |event| {
                            settings.update(|s| s.api_key = event_target_value(&event));
                        }
                    />
                </div>
                <div class="textarea-field">
                    <label>{move_tr!("story-model")}</label>
                    <input
                        type="text"
                        prop:value=move || settings.get().model
                        on:input=move |event| {
                            settings.update(|s| s.model = event_target_value(&event));
                        }
                    />
                </div>
                <div class="modal-actions">
                    <button on:click=on_save>{move_tr!("story-save")}</button>
                </div>
            </div>
        </Modal>
    }
}

// --- Story Sidebar ---

#[component]
fn StorySidebar(
    char_id: Uuid,
    stories: RwSignal<Vec<Story>>,
) -> impl IntoView {
    let current_label = Signal::derive(move || {
        // No "current" label for sidebar toggle — stories use routing
        String::new()
    });

    view! {
        <ReferenceSidebar current_label>
            <A
                href=format!("{BASE_URL}/c/{char_id}/story")
                exact=true
                attr:class="reference-nav-item story-nav-new"
            >
                {move_tr!("story-new")}
            </A>
            <For
                each=move || stories.get()
                key=|story| story.id
                let:story
            >
                <A
                    href=format!("{BASE_URL}/c/{char_id}/story/{}", story.id)
                    attr:class="reference-nav-item"
                >
                    <span class="story-nav-title">{story.title.clone()}</span>
                    <span class="story-nav-date">{story.short_date().to_string()}</span>
                </A>
            </For>
        </ReferenceSidebar>
    }
}

// --- New Story View ---

#[component]
fn NewStoryView(
    char_id: Uuid,
    stories: RwSignal<Vec<Story>>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let show_settings = RwSignal::new(false);
    let prompt = RwSignal::new(String::new());
    let streaming_text = RwSignal::new(String::new());
    let is_streaming = RwSignal::new(false);
    let error_msg = RwSignal::new(Option::<String>::None);

    let settings = Memo::new(move |_| storage::load_ai_settings());
    let has_key = move || settings.get().has_api_key();

    let build_context = move || {
        let character = store.get();
        CharacterContext {
            name: character.identity.name.clone(),
            species: character.identity.species.clone(),
            class_summary: character.class_summary(),
            level: character.level(),
            history: character.personality.history.clone(),
            personality_traits: character.personality.personality_traits.clone(),
            ideals: character.personality.ideals.clone(),
            bonds: character.personality.bonds.clone(),
            flaws: character.personality.flaws.clone(),
            notes: character.notes.clone(),
        }
    };

    let on_generate = move |_| {
        let ai_settings = settings.get_untracked();
        if !ai_settings.has_api_key() {
            return;
        }
        let user_prompt = prompt.get_untracked();
        if user_prompt.trim().is_empty() {
            return;
        }
        let context = build_context();

        is_streaming.set(true);
        error_msg.set(None);
        streaming_text.set(String::new());

        spawn_local(async move {
            let result = generate_story(
                &ai_settings,
                &context,
                &user_prompt,
                |chunk| {
                    streaming_text.update(|text| text.push_str(chunk));
                },
            )
            .await;

            is_streaming.set(false);

            match result {
                Ok(full_text) => {
                    // Auto-generate title from first line or first 50 chars
                    let title = full_text
                        .lines()
                        .next()
                        .unwrap_or("Untitled")
                        .chars()
                        .take(60)
                        .collect::<String>();

                    let story = Story::new(title, user_prompt, full_text);
                    stories.update(|list| list.insert(0, story));
                    storage::save_stories(&char_id, &stories.get_untracked());

                    // Clear prompt after successful generation
                    prompt.set(String::new());
                }
                Err(error) => {
                    error_msg.set(Some(error));
                }
            }
        });
    };

    view! {
        <div class="story-generate-view">
            // Streaming output / status area
            <div class="story-output">
                {move || {
                    let text = streaming_text.get();
                    let err = error_msg.get();
                    if let Some(error) = err {
                        Either::Left(view! {
                            <div class="story-error">
                                <p><strong>{move_tr!("story-error")}</strong></p>
                                <p>{error}</p>
                            </div>
                        })
                    } else if text.is_empty() && !is_streaming.get() {
                        Either::Right(Either::Left(view! {
                            <p class="story-placeholder">{move_tr!("story-select")}</p>
                        }))
                    } else {
                        Either::Right(Either::Right(view! {
                            <div class="story-content">
                                <pre>{text}</pre>
                            </div>
                        }))
                    }
                }}
            </div>

            // Prompt input area
            <div class="story-input">
                {move || {
                    if !has_key() {
                        Either::Left(view! {
                            <div class="story-no-key">
                                <p>{move_tr!("story-no-api-key")}</p>
                                <button on:click=move |_| show_settings.set(true)>
                                    {move_tr!("story-settings")}
                                </button>
                            </div>
                        })
                    } else {
                        Either::Right(view! {
                            <div class="story-prompt">
                                <textarea
                                    class="notes-textarea"
                                    placeholder=move_tr!("story-prompt-placeholder")
                                    prop:value=move || prompt.get()
                                    on:input=move |event| {
                                        prompt.set(event_target_value(&event));
                                    }
                                    disabled=move || is_streaming.get()
                                />
                                <div class="story-actions">
                                    <button
                                        on:click=on_generate
                                        disabled=move || is_streaming.get()
                                    >
                                        {move || if is_streaming.get() {
                                            move_tr!("story-stop")
                                        } else if error_msg.get().is_some() {
                                            move_tr!("story-retry")
                                        } else {
                                            move_tr!("story-generate")
                                        }}
                                    </button>
                                    <button
                                        class="btn-icon"
                                        title=move_tr!("story-settings")
                                        on:click=move |_| show_settings.set(true)
                                    >
                                        <Icon name="settings" size=18 />
                                    </button>
                                </div>
                            </div>
                        })
                    }
                }}
            </div>
        </div>
        <AiSettingsModal show=show_settings />
    }
}

// --- View Story ---

#[component]
fn ViewStoryView(
    char_id: Uuid,
    story_id: Uuid,
    stories: RwSignal<Vec<Story>>,
) -> impl IntoView {
    let story = Memo::new(move |_| {
        stories.get().into_iter().find(|s| s.id == story_id)
    });

    let navigate = leptos_router::hooks::use_navigate();
    let on_delete = move |_| {
        stories.update(|list| list.retain(|s| s.id != story_id));
        storage::save_stories(&char_id, &stories.get_untracked());
        navigate(&format!("{BASE_URL}/c/{char_id}/story"), Default::default());
    };

    let on_copy = move |_| {
        if let Some(story) = story.get() {
            let clipboard = web_sys::window()
                .and_then(|window| window.navigator().clipboard());
            if let Some(clipboard) = clipboard {
                let _ = clipboard.write_text(&story.content);
            }
        }
    };

    view! {
        {move || story.get().map(|story| view! {
            <div class="story-view">
                <div class="story-view-header">
                    <h2>{story.title.clone()}</h2>
                    <span class="story-view-date">{story.short_date().to_string()}</span>
                </div>
                <div class="story-view-prompt">
                    <em>{story.prompt.clone()}</em>
                </div>
                <div class="story-content">
                    <pre>{story.content.clone()}</pre>
                </div>
                <div class="story-actions">
                    <button on:click=on_copy>
                        <Icon name="copy" size=16 />
                        {move_tr!("story-copy")}
                    </button>
                    <button class="btn-danger" on:click=on_delete>
                        <Icon name="trash-2" size=16 />
                        {move_tr!("story-delete")}
                    </button>
                </div>
            </div>
        })}
    }
}

// --- Main Story Page ---

#[component]
pub fn CharacterStory() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let char_id = store.read_untracked().id;
    let stories = RwSignal::new(storage::load_stories(&char_id));
    let params = use_params::<StoryParams>();

    let story_id = move || params.get().ok().and_then(|p| p.story_id);

    view! {
        <div class="reference-page">
            <div class="reference-layout">
                <StorySidebar char_id stories />
                <main class="reference-main">
                    {move || match story_id() {
                        Some(sid) => Either::Left(view! {
                            <ViewStoryView char_id story_id=sid stories />
                        }),
                        None => Either::Right(view! {
                            <NewStoryView char_id stories />
                        }),
                    }}
                </main>
            </div>
        </div>
    }
}
```

- [ ] **Step 2: Make `ReferenceSidebar` accessible from `story.rs`**

Check that `src/pages/reference/mod.rs` exports `ReferenceSidebar` publicly. If it's already `pub use sidebar::ReferenceSidebar;` — no change needed. Otherwise add the re-export.

- [ ] **Step 3: Verify it compiles**

Run: `cargo clippy --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src/pages/character/story.rs
git commit -m "feat: implement story generation page with streaming and sidebar"
```

---

### Task 6: Minimal CSS for story page

**Files:**
- Modify: `public/styles.scss`

- [ ] **Step 1: Add story-specific styles**

Add at the end of `public/styles.scss`, before any closing brace:

```scss
// --- Story generator ---

.story-generate-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 60vh;
}

.story-output {
  flex: 1;
  overflow-y: auto;
  padding: var(--size-2);
}

.story-content pre {
  white-space: pre-wrap;
  word-wrap: break-word;
  font-family: inherit;
  margin: 0;
  line-height: 1.6;
}

.story-placeholder {
  color: var(--text-secondary);
  text-align: center;
  padding: var(--size-4);
}

.story-error {
  color: var(--danger);
  padding: var(--size-2);
}

.story-input {
  border-top: 1px solid var(--panel-border);
  padding: var(--size-2);
}

.story-prompt {
  display: flex;
  flex-direction: column;
  gap: var(--size-2);

  textarea {
    min-height: 80px;
    resize: vertical;
  }
}

.story-actions {
  display: flex;
  gap: var(--size-2);
  align-items: center;
}

.story-no-key {
  text-align: center;
  padding: var(--size-3);
  color: var(--text-secondary);

  button {
    margin-top: var(--size-2);
  }
}

.story-view-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: var(--size-2);
  padding-bottom: var(--size-2);
  border-bottom: 2px solid var(--panel-border);

  h2 {
    margin: 0;
    font-size: var(--font-size-4);
  }
}

.story-view-date {
  color: var(--text-secondary);
  font-size: var(--font-size-0);
}

.story-view-prompt {
  color: var(--text-secondary);
  margin-bottom: var(--size-3);
  padding: var(--size-2);
  background: var(--input-bg);
  border-radius: var(--radius-1);
}

.story-nav-title {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.story-nav-date {
  display: block;
  font-size: var(--font-size-00, 0.7rem);
  color: var(--text-secondary);
}
```

- [ ] **Step 2: Verify it compiles (Trunk builds SCSS)**

Run: `cargo clippy --target wasm32-unknown-unknown 2>&1 | tail -5`
Expected: no errors (SCSS is compiled by Trunk at serve/build time, not cargo)

- [ ] **Step 3: Commit**

```bash
git add public/styles.scss
git commit -m "feat: add CSS styles for story generator page"
```

---

### Task 7: Manual testing and polish

**Files:** None new — this is a manual verification task.

- [ ] **Step 1: Start the dev server**

Run: `trunk serve --port 3000 --open`

- [ ] **Step 2: Verify routing**

1. Navigate to a character
2. Click "Story" in the navbar — should see the two-column layout
3. URL should be `/c/{id}/story`

- [ ] **Step 3: Verify settings modal**

1. Click the settings gear icon
2. Enter an OpenAI API key
3. Set model to `gpt-4o-mini`
4. Click Save
5. Reload page — settings should persist

- [ ] **Step 4: Test story generation**

1. Enter a prompt in the textarea
2. Click Generate
3. Text should stream in the output area
4. After completion, story should appear in the sidebar
5. Click the story in the sidebar — should navigate to `/c/{id}/story/{story_id}`

- [ ] **Step 5: Test story view**

1. On the story view page, verify title, date, prompt, and content display
2. Click Copy — text should copy to clipboard
3. Click Delete — story should be removed, navigate back to `/c/{id}/story`

- [ ] **Step 6: Test error states**

1. Clear the API key, try generating — should show "configure API key" message
2. Set an invalid API key, try generating — should show error message with retry button

- [ ] **Step 7: Test mobile layout**

1. Resize browser to < 768px width
2. Sidebar should collapse
3. Toggle button should show/hide story list

- [ ] **Step 8: Run linter and formatter**

Run: `cargo clippy --target wasm32-unknown-unknown && cargo +nightly fmt`
Fix any issues.

- [ ] **Step 9: Final commit (if any fixes)**

```bash
git add -A
git commit -m "fix: polish story generator after manual testing"
```
