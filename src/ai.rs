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
