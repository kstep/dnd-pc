use std::collections::{BTreeMap, HashSet};

use leptos::prelude::*;
use serde::Deserialize;

use crate::rules::utils::fetch_json;

/// Cached raw (unlocalized) data alongside its relative path, so we can
/// re-apply a different locale without refetching the data file.
struct RawEntry<T> {
    data: T,
    /// Relative path used to construct locale URLs (e.g. "classes/wizard.json").
    path: Box<str>,
}

pub struct FetchCache<T: Send + Sync + 'static> {
    data: RwSignal<BTreeMap<Box<str>, T>>,
    raw: RwSignal<BTreeMap<Box<str>, RawEntry<T>>>,
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
            raw: RwSignal::new(BTreeMap::new()),
            pending: RwSignal::new(HashSet::new()),
        }
    }

    pub fn clear(&self) {
        self.data.update(|m| m.clear());
        self.raw.update(|m| m.clear());
        self.pending.update(|s| s.clear());
    }
}

impl<T: Clone + for<'de> Deserialize<'de> + Send + Sync + 'static> FetchCache<T> {
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
    /// The raw (unlocalized) data is stored so locale can be re-applied later
    /// without refetching the data file.
    pub fn fetch_localized<L: for<'de> Deserialize<'de> + 'static>(
        &self,
        name: &str,
        path: &str,
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
        let raw = self.raw;
        let pending = self.pending;
        let path: Box<str> = path.into();
        leptos::task::spawn_local(async move {
            // Fetch data and locale in parallel
            let (data_result, locale_result) =
                futures::join!(fetch_json::<T>(&data_url), fetch_json::<L>(&locale_url));
            pending.update_untracked(|s| {
                s.remove(&name);
            });
            match data_result {
                Ok(val) => {
                    // Store raw copy for future relocalization
                    raw.update_untracked(|m| {
                        m.insert(name.clone(), RawEntry { data: val.clone(), path });
                    });
                    // Apply locale and store localized version
                    let mut localized = val;
                    match locale_result {
                        Ok(locale) => apply(&mut localized, &locale),
                        Err(error) => {
                            log::warn!("Failed to fetch locale for {error_ctx}: {error}");
                        }
                    }
                    data.update(|m| {
                        m.insert(name, localized);
                    });
                }
                Err(error) => {
                    log::error!("Failed to fetch {error_ctx}: {error}");
                }
            }
        });
    }

    /// Re-apply locale to all cached entries without refetching data files.
    /// Clones each raw entry, fetches the new locale file, applies it, and
    /// updates the localized cache.
    pub fn relocalize<L: for<'de> Deserialize<'de> + 'static>(
        &self,
        locale_url_fn: impl Fn(&str) -> String,
        apply: fn(&mut T, &L),
        error_ctx: &'static str,
    ) {
        // Collect raw entries: (name, cloned_raw_data, new_locale_url)
        let entries: Vec<(Box<str>, T, String)> = self.raw.read_untracked().iter().map(|(name, entry)| {
            (name.clone(), entry.data.clone(), locale_url_fn(&entry.path))
        }).collect();

        let data = self.data;
        for (name, raw_data, locale_url) in entries {
            leptos::task::spawn_local(async move {
                let mut val = raw_data;
                match fetch_json::<L>(&locale_url).await {
                    Ok(locale) => apply(&mut val, &locale),
                    Err(error) => {
                        log::warn!("Failed to fetch locale for {error_ctx}: {error}");
                    }
                }
                data.update(|m| {
                    m.insert(name, val);
                });
            });
        }
    }
}

/// A pair of (data_url, locale_url) for localized fetching.
pub type LocalizedUrls = (String, String);

/// Trait for unified access to definition caches (class, race, background).
/// Newtype wrappers implement the 4 required methods; default methods
/// eliminate the repeated has/with/with_tracked/fetch/fetch_tracked
/// boilerplate.
pub trait DefinitionStore {
    type Definition: Clone + for<'de> serde::Deserialize<'de> + Send + Sync + 'static;
    type Locale: for<'de> serde::Deserialize<'de> + 'static;

    fn cache(&self) -> FetchCache<Self::Definition>;
    fn index_urls(&self, name: &str) -> Option<LocalizedUrls>;
    fn index_urls_tracked(&self, name: &str) -> Option<LocalizedUrls>;
    fn apply_locale(def: &mut Self::Definition, locale: &Self::Locale);
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
            let path = extract_path(&data_url);
            self.cache().fetch_localized(
                name,
                &path,
                data_url,
                locale_url,
                Self::apply_locale,
                Self::type_label(),
            );
        }
    }

    fn fetch_tracked(&self, name: &str) {
        if let Some((data_url, locale_url)) = self.index_urls_tracked(name) {
            let path = extract_path(&data_url);
            self.cache().fetch_localized(
                name,
                &path,
                data_url,
                locale_url,
                Self::apply_locale,
                Self::type_label(),
            );
        }
    }
}

/// Extract the relative path from a data URL (strips the `{BASE_URL}/data/`
/// prefix).
fn extract_path(data_url: &str) -> String {
    use crate::BASE_URL;
    let prefix = format!("{BASE_URL}/data/");
    data_url
        .strip_prefix(&prefix)
        .unwrap_or(data_url)
        .to_string()
}
