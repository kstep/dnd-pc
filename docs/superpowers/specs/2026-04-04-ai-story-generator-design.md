# AI Story Generator — Design Spec

## Overview

Add an AI-powered story generator to the D&D character sheet app. Players write a short prompt describing what happened between game sessions, and the app generates a narrative story based on the character's identity, personality, and backstory. Stories are streamed in real-time from the OpenAI API, saved locally, and browsable per character.

## Data Model

### Story

Stored separately from `Character` in localStorage at `dnd_pc_stories_{char_uuid}` as `Vec<Story>`. Future: Firestore subcollection `characters/{uid}_{char_id}/stories/{story_id}`.

```rust
struct Story {
    id: Uuid,
    title: String,
    prompt: String,
    content: String,
    created_at: String,  // ISO 8601
}
```

### AiSettings

Stored in localStorage at `dnd_pc_ai_settings`.

```rust
struct AiSettings {
    provider: AiProvider,
    api_key: String,
    model: String,
}

enum AiProvider {
    OpenAI,
    // TODO: Anthropic (requires CORS proxy or backend)
}

impl AiProvider {
    fn default_model(&self) -> &str;  // OpenAI -> "gpt-4o-mini"
    fn api_url(&self) -> &str;        // OpenAI -> "https://api.openai.com/v1/chat/completions"
}
```

Only OpenAI is implemented. `AiProvider` enum is extensible for future providers.

## AI Client (`src/ai.rs`)

### Interface

```rust
async fn generate_story(
    settings: &AiSettings,
    context: CharacterContext,
    prompt: &str,
    on_chunk: impl Fn(&str),
) -> Result<String, String>
```

### CharacterContext

Automatically assembled from `Store<Character>`:

- Name, species, class (with subclass), level
- Backstory (history)
- Personality traits, ideals, bonds, flaws
- Recent notes (truncated if too long)

No stats or feature details — only narrative fields relevant to storytelling.

### System Prompt

```
You are a creative D&D storyteller. Write a short story about what
the character did between game sessions, based on their details and
the player's prompt. Write in the same language as the player's prompt.
```

### Streaming

- POST to OpenAI with `stream: true`
- Parse SSE via `web_sys::ReadableStreamDefaultReader` from response body
- Each `data: {...}` chunk parsed for `choices[0].delta.content`
- Text chunks passed to `on_chunk` callback for real-time UI updates

## Routing

Nested under existing `ParentRoute` for `/c/:id`:

- `/c/:id/story` — new story generation page
- `/c/:id/story/:story_id` — view saved story

Link "Story" added to character navbar (alongside Sheet / Summary).

## Page Layout

Two-column layout reusing `.reference-layout` pattern from reference pages.

### Sidebar (`.reference-sidebar`)

- List of saved stories: each is an `<A>` link to `/c/:id/story/:story_id`
- Each entry shows title + date
- On mobile: collapsible via `reference-nav-toggle` pattern

### Main: New Story (`/c/:id/story`)

- **Bottom:** `.textarea-field` for user prompt + "Generate" button + settings gear icon
- **Above:** streaming output area — text appears in real-time during generation
- "Generate" becomes "Stop" during streaming
- If no API key configured: show message with link to open settings modal

### Main: View Story (`/c/:id/story/:story_id`)

- Story text displayed read-only
- Buttons: delete, copy to clipboard

### Settings Modal

Opened via gear icon next to Generate button. Fields:

- API key (password input)
- Model (text input, default: `gpt-4o-mini`)

Reuses existing `.modal` component and patterns.

## UI States

| State | Generate button | Textarea | Output area |
|-------|----------------|----------|-------------|
| Idle | "Generate" enabled | Enabled | Empty or placeholder |
| Streaming | "Stop" enabled | Disabled | Text appearing |
| Error | "Retry" enabled | Enabled | Error message |
| No API key | Disabled | Disabled | "Configure API key" message |

## CSS Reuse

All UI built from existing classes:

- `.reference-layout`, `.reference-sidebar`, `.reference-nav-item`, `.reference-main` — page structure
- `.textarea-field` — prompt input
- `.panel` — story display sections
- `.modal`, `.modal-header` — settings dialog
- Standard `button` styles — Generate/Stop/Delete/Copy
- `.summary-section` — content blocks

Minimal new CSS expected (mainly for streaming text area if needed).

## Storage

- Stories: `dnd_pc_stories_{char_uuid}` in localStorage
- AI settings: `dnd_pc_ai_settings` in localStorage
- No cloud sync for stories in v1 (future: Firestore subcollection)

## Future Extensions

- Anthropic Claude provider (requires CORS proxy or backend)
- Chat format: `Vec<Message>` conversation model for iterative story refinement
- Cloud sync for stories via Firestore subcollection
- Checkbox to include character stats/features in context
- Story editing after generation
