use std::collections::{BTreeMap, HashSet};

use futures::future::join_all;
use leptos::prelude::*;
use serde::Deserialize;

use crate::{BASE_URL, rules::utils::fetch_json};

/// Raw definition + its relative path (e.g. "classes/wizard.json").
type RawEntry<T> = (T, Box<str>);

pub struct FetchCache<T: Clone + Send + Sync + 'static> {
    /// Raw (unlocalized) definitions + relative paths. Entries added on demand,
    /// never cleared on locale change.
    raw: RwSignal<BTreeMap<Box<str>, RawEntry<T>>>,
    /// Merged result (raw + locale applied). What consumers read via Deref.
    data: RwSignal<BTreeMap<Box<str>, T>>,
    /// Dedup for in-flight data fetches.
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
            raw: RwSignal::new(BTreeMap::new()),
            data: RwSignal::new(BTreeMap::new()),
            pending: RwSignal::new(HashSet::new()),
        }
    }

    pub fn clear(&self) {
        self.raw.update(|m| m.clear());
        self.data.update(|m| m.clear());
        self.pending.update(|s| s.clear());
    }
}

impl<T: Clone + for<'de> Deserialize<'de> + Send + Sync + 'static> FetchCache<T> {
    /// Fetch a resource from data URL + locale URL in parallel, store raw data
    /// for future locale switches, and cache the localized result.
    pub fn fetch_with_initial_locale<L: for<'de> Deserialize<'de> + 'static>(
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

        let raw = self.raw;
        let data = self.data;
        let pending = self.pending;
        let path: Box<str> = path.into();
        leptos::task::spawn_local(async move {
            let (data_result, locale_result) =
                futures::join!(fetch_json::<T>(&data_url), fetch_json::<L>(&locale_url));
            pending.update_untracked(|s| {
                s.remove(&name);
            });
            match data_result {
                Ok(val) => {
                    // Store raw for future locale switches
                    raw.update_untracked(|m| {
                        m.insert(name.clone(), (val.clone(), path));
                    });
                    // Apply locale and store merged result
                    let mut localized = val;
                    if let Ok(locale) = locale_result {
                        apply(&mut localized, &locale);
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

    /// Fetch locale files for all cached entries in parallel.
    /// Only clones paths (cheap), not definitions.
    pub async fn fetch_locale<L: for<'de> Deserialize<'de> + 'static>(
        &self,
        locale: &str,
    ) -> Vec<(Box<str>, Option<L>)> {
        // Collect only names + paths (cheap Box<str> clones, no definition clones)
        let paths: Vec<(Box<str>, Box<str>)> = self
            .raw
            .with_untracked(|m| m.iter().map(|(n, (_, p))| (n.clone(), p.clone())).collect());
        let futs = paths.into_iter().map(|(name, path)| {
            let url = format!("{BASE_URL}/{locale}/{path}");
            async move { (name, fetch_json::<L>(&url).await.ok()) }
        });
        join_all(futs).await
    }

    /// Apply fetched locale data to cached entries.
    /// Clones from raw first so stale labels from a previous locale are
    /// cleared even when the new locale map lacks a corresponding entry.
    pub fn apply_locale_batch<L>(
        &self,
        results: &[(Box<str>, Option<L>)],
        apply: fn(&mut T, &L),
        notify: bool,
    ) {
        let raw = self.raw.read_untracked();
        let update_fn = |m: &mut BTreeMap<Box<str>, T>| {
            for (name, locale_opt) in results {
                // Reset to raw (unlocalized) data before applying new locale.
                if let Some((raw_def, _)) = raw.get(name) {
                    m.insert(name.clone(), raw_def.clone());
                }
                if let Some(def) = m.get_mut(name)
                    && let Some(locale) = locale_opt
                {
                    apply(def, locale);
                }
            }
        };
        if notify {
            self.data.update(update_fn);
        } else {
            self.data.update_untracked(update_fn);
        }
    }
}

/// Trait for unified access to definition caches (class, species, background).
pub trait DefinitionStore {
    type Definition: Clone + for<'de> serde::Deserialize<'de> + Send + Sync + 'static;
    type Locale: for<'de> serde::Deserialize<'de> + 'static;

    fn cache(&self) -> FetchCache<Self::Definition>;
    fn data_url(&self, name: &str) -> Option<String>;
    fn locale_url(&self, name: &str) -> Option<String>;
    fn data_url_tracked(&self, name: &str) -> Option<String>;
    fn locale_url_tracked(&self, name: &str) -> Option<String>;
    fn path(&self, name: &str) -> Option<String>;
    fn path_tracked(&self, name: &str) -> Option<String>;
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
        if let Some(path) = self.path(name)
            && let Some(data_url) = self.data_url(name)
            && let Some(locale_url) = self.locale_url(name)
        {
            self.cache().fetch_with_initial_locale(
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
        if let Some(path) = self.path_tracked(name)
            && let Some(data_url) = self.data_url_tracked(name)
            && let Some(locale_url) = self.locale_url_tracked(name)
        {
            self.cache().fetch_with_initial_locale(
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
