use std::collections::{BTreeMap, HashSet};

use leptos::prelude::*;
use serde::Deserialize;

use crate::rules::utils::fetch_json;

pub struct FetchCache<T: Send + Sync + 'static> {
    data: RwSignal<BTreeMap<Box<str>, T>>,
    pending: RwSignal<HashSet<Box<str>>>,
}

impl<T: Send + Sync + 'static> Clone for FetchCache<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Send + Sync + 'static> Copy for FetchCache<T> {}

impl<T: Send + Sync + 'static> std::ops::Deref for FetchCache<T> {
    type Target = RwSignal<BTreeMap<Box<str>, T>>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: Send + Sync + 'static> FetchCache<T> {
    pub fn new() -> Self {
        Self {
            data: RwSignal::new(BTreeMap::new()),
            pending: RwSignal::new(HashSet::new()),
        }
    }

    pub fn clear(&self) {
        self.data.update(|m| m.clear());
        self.pending.update(|s| s.clear());
    }
}

impl<T: for<'de> Deserialize<'de> + Send + Sync + 'static> FetchCache<T> {
    /// Fetch a resource if it's not already cached or in-flight.
    /// Returns immediately if the resource is cached or a fetch is pending.
    pub fn fetch(&self, name: &str, url: String, error_ctx: &'static str) {
        if self.data.read_untracked().contains_key(name) {
            return;
        }
        if self.pending.read_untracked().contains(name) {
            return;
        }

        let name: Box<str> = name.into();
        self.pending.update_untracked(|s| s.insert(name.clone()));

        let data = self.data;
        let pending = self.pending;
        leptos::task::spawn_local(async move {
            let result = fetch_json::<T>(&url).await;
            pending.update_untracked(|s| {
                s.remove(&name);
            });
            match result {
                Ok(val) => {
                    data.update(|m| {
                        m.insert(name, val);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch {error_ctx}: {error}");
                }
            }
        });
    }

    /// Fetch a resource from a data URL + locale URL, merge them, and cache.
    /// Both URLs are fetched; if the locale fetch fails, the data is cached
    /// without locale (labels/descriptions will be empty).
    pub fn fetch_localized<L: for<'de> Deserialize<'de> + 'static>(
        &self,
        name: &str,
        data_url: String,
        locale_url: String,
        apply: fn(&mut T, &L),
        error_ctx: &'static str,
    ) {
        if self.data.read_untracked().contains_key(name) {
            return;
        }
        if self.pending.read_untracked().contains(name) {
            return;
        }

        let name: Box<str> = name.into();
        self.pending.update_untracked(|s| s.insert(name.clone()));

        let data = self.data;
        let pending = self.pending;
        leptos::task::spawn_local(async move {
            let data_result = fetch_json::<T>(&data_url).await;
            pending.update_untracked(|s| {
                s.remove(&name);
            });
            match data_result {
                Ok(mut val) => {
                    // Best-effort locale application
                    match fetch_json::<L>(&locale_url).await {
                        Ok(locale) => apply(&mut val, &locale),
                        Err(error) => {
                            log::warn!("Failed to fetch locale for {error_ctx}: {error}");
                        }
                    }
                    data.update(|m| {
                        m.insert(name, val);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch {error_ctx}: {error}");
                }
            }
        });
    }
}

/// A pair of (data_url, locale_url) for localized fetching.
pub type LocalizedUrls = (String, String);

/// Trait for unified access to definition caches (class, race, background).
/// Newtype wrappers implement the 4 required methods; default methods
/// eliminate the repeated has/with/with_tracked/fetch/fetch_tracked
/// boilerplate.
pub trait DefinitionStore {
    type Definition: for<'de> serde::Deserialize<'de> + Send + Sync + 'static;
    type Locale: for<'de> serde::Deserialize<'de> + 'static;

    fn cache(&self) -> FetchCache<Self::Definition>;
    fn index_urls(&self, name: &str) -> Option<LocalizedUrls>;
    fn index_urls_tracked(&self, name: &str) -> Option<LocalizedUrls>;
    fn apply(def: &mut Self::Definition, locale: &Self::Locale);
    fn type_label() -> &'static str;

    fn has(&self, name: &str) -> bool {
        self.cache().read_untracked().contains_key(name)
    }

    fn with<R>(&self, name: &str, f: impl FnOnce(&Self::Definition) -> R) -> Option<R> {
        self.cache().read_untracked().get(name).map(f)
    }

    fn with_tracked<R>(&self, name: &str, f: impl FnOnce(&Self::Definition) -> R) -> Option<R> {
        self.cache().read().get(name).map(f)
    }

    fn fetch(&self, name: &str) {
        if let Some((data_url, locale_url)) = self.index_urls(name) {
            self.cache().fetch_localized(
                name,
                data_url,
                locale_url,
                Self::apply,
                Self::type_label(),
            );
        }
    }

    fn fetch_tracked(&self, name: &str) {
        if let Some((data_url, locale_url)) = self.index_urls_tracked(name) {
            self.cache().fetch_localized(
                name,
                data_url,
                locale_url,
                Self::apply,
                Self::type_label(),
            );
        }
    }
}
