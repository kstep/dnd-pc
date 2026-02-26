use std::collections::HashMap;

use leptos::prelude::*;
use serde::Deserialize;

use crate::{
    BASE_URL,
    model::{Ability, Proficiency},
};

// --- JSON types ---

#[derive(Debug, Clone, Deserialize)]
pub struct ClassIndex {
    pub classes: Vec<ClassIndexEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassIndexEntry {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub prerequisites: Vec<Ability>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassDefinition {
    pub name: String,
    pub description: String,
    pub hit_die: u16,
    #[serde(default)]
    pub casting_ability: Option<Ability>,
    #[serde(default)]
    pub proficiencies: Vec<Proficiency>,
    #[serde(default)]
    pub saving_throws: Vec<Ability>,
    #[serde(default)]
    pub features: Vec<ClassFeature>,
    #[serde(default)]
    pub spells: Vec<ClassSpell>,
    #[serde(default)]
    pub levels: Vec<ClassLevelRules>,
    #[serde(default)]
    pub subclasses: Vec<SubclassDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub features: Vec<ClassFeature>,
    #[serde(default)]
    pub spells: Vec<ClassSpell>,
    #[serde(default)]
    pub levels: Vec<SubclassLevelRules>,
}

impl SubclassDefinition {
    pub fn min_level(&self) -> u32 {
        self.levels.first().map_or(1, |lr| lr.level)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubclassLevelRules {
    pub level: u32,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassFeature {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassSpell {
    pub name: String,
    pub level: u32,
    pub description: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClassLevelRules {
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub spell_slots: Option<Vec<u32>>,
    #[serde(default)]
    pub cantrips_known: Option<u32>,
    #[serde(default)]
    pub spells_known: Option<u32>,
    #[serde(default)]
    pub sorcery_points: Option<u32>,
}

// --- Registry ---

#[derive(Clone, Copy)]
pub struct RulesRegistry {
    pub class_index: LocalResource<Result<ClassIndex, String>>,
    pub class_cache: RwSignal<HashMap<String, ClassDefinition>>,
}

async fn fetch_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let resp = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|error| format!("fetch error: {error}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json()
        .await
        .map_err(|error| format!("parse error: {error}"))
}

impl Default for RulesRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RulesRegistry {
    pub fn new() -> Self {
        let index_url = format!("{BASE_URL}/classes/index.json");
        let class_index = LocalResource::new(move || {
            let url = index_url.clone();
            async move { fetch_json::<ClassIndex>(&url).await }
        });

        Self {
            class_index,
            class_cache: RwSignal::new(HashMap::new()),
        }
    }

    pub fn with_class_entries<R>(&self, f: impl FnOnce(&[ClassIndexEntry]) -> R) -> R {
        let guard = self.class_index.read();
        let entries = guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|idx| idx.classes.as_slice());
        f(entries.unwrap_or(&[]))
    }

    pub fn get_class(&self, name: &str) -> Option<ClassDefinition> {
        self.class_cache.read().get(name).cloned()
    }

    pub fn fetch_class(&self, name: &str) {
        if self.class_cache.read().contains_key(name) {
            return;
        }

        let url = {
            let guard = self.class_index.read();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index.classes.iter().find(|e| e.name == name) {
                Some(entry) => format!("{BASE_URL}/{}", entry.url),
                None => return,
            }
        };

        let cache = self.class_cache;
        let name = name.to_string();
        leptos::task::spawn_local(async move {
            match fetch_json::<ClassDefinition>(&url).await {
                Ok(def) => {
                    cache.update(|m| {
                        m.insert(name, def);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch class definition: {error}");
                }
            }
        });
    }
}
