use std::collections::{BTreeMap, HashSet};

use leptos::prelude::*;
use serde::Deserialize;

use crate::{
    demap::Named,
    rules::{
        locale::{LocaleMap, LocalizedText},
        utils::fetch_json,
    },
};

/// Lazy per-name fetch cache. Backs the loaded class/species/background
/// definitions and per-class spell name lists. Entries are inserted on demand
/// and never cleared on locale change — locale lives in a parallel
/// `FetchCache<LocaleMap>`.
pub struct FetchCache<T: Clone + Send + Sync + 'static> {
    /// Loaded entries, keyed by name. What consumers read via `Deref`.
    data: RwSignal<BTreeMap<Box<str>, T>>,
    /// Dedup for in-flight fetches.
    pending: RwSignal<HashSet<Box<str>>>,
}

impl<T: Clone + Send + Sync + 'static> Clone for FetchCache<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Clone + Send + Sync + 'static> Copy for FetchCache<T> {}

impl<T: Clone + Send + Sync + 'static> std::ops::Deref for FetchCache<T> {
    type Target = RwSignal<BTreeMap<Box<str>, T>>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Clone + Send + Sync + 'static> FetchCache<T> {
    pub fn new() -> Self {
        Self {
            data: RwSignal::new(BTreeMap::new()),
            pending: RwSignal::new(HashSet::new()),
        }
    }

    pub fn is_pending(&self) -> bool {
        !self.pending.read().is_empty()
    }

    pub fn clear(&self) {
        self.data.update(|m| m.clear());
        self.pending.update(|s| s.clear());
    }
}

impl<T: Clone + for<'de> Deserialize<'de> + Send + Sync + 'static> FetchCache<T> {
    /// Fetch a single resource by name. No-op if already loaded or fetch in
    /// flight. The cached entry survives locale changes — locale text lives
    /// in a parallel `FetchCache<LocaleMap>`.
    pub fn fetch(&self, name: &str, data_url: String, error_ctx: &'static str) {
        if self.data.read_untracked().contains_key(name)
            || self.pending.read_untracked().contains(name)
        {
            return;
        }
        let name: Box<str> = name.into();
        self.pending.update(|s| {
            s.insert(name.clone());
        });
        let data = self.data;
        let pending = self.pending;
        leptos::task::spawn_local(async move {
            let result = fetch_json::<T>(&data_url).await;
            pending.update(|s| {
                s.remove(&name);
            });
            match result {
                Ok(value) => {
                    data.update(|m| {
                        m.insert(name, value);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch {error_ctx}: {error}");
                }
            }
        });
    }
}

/// Pairs a per-name data `FetchCache` with its locale overlay. Mirrors the
/// LocalResource-based `LocalizedIndex` (in registry.rs) for the lazy
/// per-name flavor used by class/species/background definitions.
pub struct LocalizedCache<T: Clone + Send + Sync + 'static> {
    pub(super) data: FetchCache<T>,
    pub(super) locale: FetchCache<LocaleMap>,
}

impl<T: Clone + Send + Sync + 'static> Clone for LocalizedCache<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Clone + Send + Sync + 'static> Copy for LocalizedCache<T> {}

impl<T: Clone + Send + Sync + 'static> std::ops::Deref for LocalizedCache<T> {
    type Target = FetchCache<T>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Clone + Send + Sync + 'static> LocalizedCache<T> {
    pub fn new() -> Self {
        Self {
            data: FetchCache::new(),
            locale: FetchCache::new(),
        }
    }

    pub fn is_pending(&self) -> bool {
        self.data.is_pending() || self.locale.is_pending()
    }

    /// Drop locale entries — used on locale change. Data survives.
    pub fn clear_locale(&self) {
        self.locale.clear();
    }
}

/// Trait for unified access to definition caches (class, species, background).
/// Each store wraps a `LocalizedCache<Definition>`; consumers obtain a
/// `LocalizedText` wrapper via `lookup()`.
///
/// Tracked methods (`has`, `with`, `lookup`, `fetch`) subscribe the calling
/// reactive context to data + locale; `_untracked` variants don't. Matches
/// the leptos `signal.get()` / `get_untracked()` convention and the
/// equivalent `LocalizedIndex` API.
pub trait DefinitionStore {
    type Definition: Clone + Named + for<'de> serde::Deserialize<'de> + Send + Sync + 'static;

    fn cache(&self) -> LocalizedCache<Self::Definition>;
    fn data_url(&self, name: &str) -> Option<String>;
    fn locale_url(&self, name: &str) -> Option<String>;
    fn data_url_untracked(&self, name: &str) -> Option<String>;
    fn locale_url_untracked(&self, name: &str) -> Option<String>;
    fn type_label() -> &'static str;

    fn has(&self, name: &str) -> bool {
        self.cache().data.read().contains_key(name)
    }

    fn has_untracked(&self, name: &str) -> bool {
        self.cache().data.read_untracked().contains_key(name)
    }

    fn with<R>(&self, name: &str, f: impl FnOnce(&Self::Definition) -> R) -> Option<R> {
        self.cache().data.read().get(name).map(f)
    }

    fn with_untracked<R>(&self, name: &str, f: impl FnOnce(&Self::Definition) -> R) -> Option<R> {
        self.cache().data.read_untracked().get(name).map(f)
    }

    fn lookup<R>(
        &self,
        name: &str,
        f: impl FnOnce(LocalizedText<'_, Self::Definition, LocaleMap>) -> R,
    ) -> Option<R> {
        let cache = self.cache();
        let data_guard = cache.data.read();
        let def = data_guard.get(name)?;
        let locale_guard = cache.locale.read();
        let locale = locale_guard.get(name);
        Some(f(LocalizedText { data: def, locale }))
    }

    fn lookup_untracked<R>(
        &self,
        name: &str,
        f: impl FnOnce(LocalizedText<'_, Self::Definition, LocaleMap>) -> R,
    ) -> Option<R> {
        let cache = self.cache();
        let data_guard = cache.data.read_untracked();
        let def = data_guard.get(name)?;
        let locale_guard = cache.locale.read_untracked();
        let locale = locale_guard.get(name);
        Some(f(LocalizedText { data: def, locale }))
    }

    fn fetch(&self, name: &str) {
        let cache = self.cache();
        if let Some(data_url) = self.data_url(name) {
            cache.data.fetch(name, data_url, Self::type_label());
        }
        if let Some(locale_url) = self.locale_url(name) {
            cache.locale.fetch(name, locale_url, "locale overlay");
        }
    }

    fn fetch_untracked(&self, name: &str) {
        let cache = self.cache();
        if let Some(data_url) = self.data_url_untracked(name) {
            cache.data.fetch(name, data_url, Self::type_label());
        }
        if let Some(locale_url) = self.locale_url_untracked(name) {
            cache.locale.fetch(name, locale_url, "locale overlay");
        }
    }
}
