use std::collections::{BTreeMap, HashSet};

use leptos::prelude::*;
use serde::Deserialize;

use crate::{
    demap::Named,
    rules::{
        locale::{LocaleMap, LocalizedText},
        packages::{DefsIndex, PackageMerge},
        registry::LocalizedIndex,
        utils::fetch_merged_json,
    },
};

/// Lazy per-name fetch cache. Backs the per-class spell name lists. Entries
/// are inserted on demand.
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
    /// Fetch and union one URL per package into a single cache entry.
    /// No-op if already loaded or fetch in flight.
    pub fn fetch_merged(&self, name: &str, urls: Vec<String>, error_ctx: &'static str)
    where
        T: PackageMerge + Default,
    {
        if self.data.read_untracked().contains_key(name)
            || self.pending.read_untracked().contains(name)
        {
            return;
        }
        let name: Box<str> = name.into();
        self.pending.update(|pending| {
            pending.insert(name.clone());
        });
        let data = self.data;
        let pending = self.pending;
        leptos::task::spawn_local(async move {
            let result = fetch_merged_json::<T>(&urls).await;
            pending.update(|set| {
                set.remove(&name);
            });
            match result {
                Ok(value) => data.update(|map| {
                    map.insert(name, value);
                }),
                Err(error) => log::error!("Failed to fetch {error_ctx}: {error}"),
            }
        });
    }
}

/// Unified read access to the merged class/species/background definitions.
/// Definitions are eager whole-file indexes — no per-name fetching.
///
/// Tracked methods (`has`, `with`, `lookup`) subscribe the calling reactive
/// context to data + locale; `_untracked` variants don't. Matches the leptos
/// `signal.get()` / `get_untracked()` convention.
pub trait DefinitionStore {
    type Definition: Clone + Named + for<'de> serde::Deserialize<'de> + Send + Sync + 'static;

    fn index(&self) -> LocalizedIndex<DefsIndex<Self::Definition>, LocaleMap>;

    fn has(&self, name: &str) -> bool {
        self.index()
            .with_data(|defs| defs.is_some_and(|index| index.0.contains_key(name)))
    }

    fn has_untracked(&self, name: &str) -> bool {
        self.index()
            .with_data_untracked(|defs| defs.is_some_and(|index| index.0.contains_key(name)))
    }

    fn with<R>(&self, name: &str, f: impl FnOnce(&Self::Definition) -> R) -> Option<R> {
        self.index()
            .with_data(|defs| defs.and_then(|index| index.0.get(name).map(f)))
    }

    fn with_untracked<R>(&self, name: &str, f: impl FnOnce(&Self::Definition) -> R) -> Option<R> {
        self.index()
            .with_data_untracked(|defs| defs.and_then(|index| index.0.get(name).map(f)))
    }

    fn lookup<R>(
        &self,
        name: &str,
        f: impl FnOnce(LocalizedText<'_, Self::Definition, LocaleMap>) -> R,
    ) -> Option<R> {
        self.index()
            .with(|index, locale| {
                index
                    .0
                    .get(name)
                    .map(|def| f(LocalizedText { data: def, locale }))
            })
            .flatten()
    }

    fn lookup_untracked<R>(
        &self,
        name: &str,
        f: impl FnOnce(LocalizedText<'_, Self::Definition, LocaleMap>) -> R,
    ) -> Option<R> {
        self.index()
            .with_untracked(|index, locale| {
                index
                    .0
                    .get(name)
                    .map(|def| f(LocalizedText { data: def, locale }))
            })
            .flatten()
    }
}
