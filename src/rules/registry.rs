use std::collections::BTreeMap;

use leptos::prelude::*;

use super::{
    background::BackgroundDefinition,
    cache::{DefinitionStore, FetchCache},
    class::ClassDefinition,
    feature::{ChoiceOption, FeatureDefinition, FieldKind},
    index::{BackgroundIndexEntry, ClassIndexEntry, Index, RaceIndexEntry, SpellIndexEntry},
    labels,
    race::RaceDefinition,
    resolve,
    spells::{SpellDefinition, SpellList, SpellMap},
    utils::fetch_json,
};
use crate::{
    BASE_URL,
    model::{
        ActiveEffect, Character, CharacterIdentity, ClassLevel, EffectsIndex, FeatureField,
        FeatureSource, FreeUses,
    },
};

// ---- DefinitionStore newtype wrappers ----

pub struct ClassDefs(RulesRegistry);
pub struct RaceDefs(RulesRegistry);
pub struct BackgroundDefs(RulesRegistry);

macro_rules! impl_definition_store {
    ($wrapper:ty, $def:ty, $cache:ident, $index_field:ident, $label:expr) => {
        impl DefinitionStore for $wrapper {
            type Definition = $def;

            fn cache(&self) -> FetchCache<$def> {
                self.0.$cache
            }

            fn index_url(&self, name: &str) -> Option<String> {
                self.0
                    .resolve_index_url(name, |idx| &idx.$index_field, false)
            }

            fn index_url_tracked(&self, name: &str) -> Option<String> {
                self.0
                    .resolve_index_url(name, |idx| &idx.$index_field, true)
            }

            fn type_label() -> &'static str {
                $label
            }
        }
    };
}

impl_definition_store!(
    ClassDefs,
    ClassDefinition,
    class_cache,
    classes,
    "class definition"
);
impl_definition_store!(
    RaceDefs,
    RaceDefinition,
    race_cache,
    races,
    "race definition"
);
impl_definition_store!(
    BackgroundDefs,
    BackgroundDefinition,
    background_cache,
    backgrounds,
    "background definition"
);

macro_rules! index_accessors {
    ($($method:ident, $label_method:ident, $field:ident, $entry:ty);+ $(;)?) => {
        $(
            pub fn $method<R>(
                &self,
                f: impl FnOnce(&BTreeMap<Box<str>, $entry>) -> R,
            ) -> R {
                static EMPTY: BTreeMap<Box<str>, $entry> = BTreeMap::new();
                self.with_index_field(|idx| &idx.$field, &EMPTY, f)
            }

            pub fn $label_method(&self, name: &str) -> String {
                self.$method(|e| label_by_name(e, name))
            }
        )+
    };
}

// ---- RulesRegistry ----

#[derive(Clone, Copy)]
pub struct RulesRegistry {
    locale: Signal<String>,
    class_index: LocalResource<Result<Index, String>>,
    pub(super) class_cache: FetchCache<ClassDefinition>,
    pub(super) race_cache: FetchCache<RaceDefinition>,
    pub(super) background_cache: FetchCache<BackgroundDefinition>,
    spell_list_cache: FetchCache<SpellMap>,
    effects_index: LocalResource<Result<EffectsIndex, String>>,
}

impl RulesRegistry {
    // ---- Index-based methods (stay on RulesRegistry) ----

    index_accessors! {
        with_class_entries,      class_label_by_name,      classes,      ClassIndexEntry;
        with_race_entries,       race_label_by_name,       races,        RaceIndexEntry;
        with_background_entries, background_label_by_name,  backgrounds,  BackgroundIndexEntry;
        with_spell_entries,      spell_label_by_name,       spells,       SpellIndexEntry;
    }

    pub fn new(i18n: leptos_fluent::I18n) -> Self {
        let locale = Signal::derive(move || i18n.language.get().id.to_string());

        let class_index = LocalResource::new(move || {
            let locale = locale.get();
            let url = format!("{BASE_URL}/{locale}/index.json");
            async move { fetch_json::<Index>(&url).await }
        });

        let effects_index = LocalResource::new(move || {
            let locale = locale.get();
            let url = format!("{BASE_URL}/{locale}/effects.json");
            async move { fetch_json::<EffectsIndex>(&url).await }
        });

        let class_cache = FetchCache::new();
        let race_cache = FetchCache::new();
        let background_cache = FetchCache::new();
        let spell_list_cache = FetchCache::new();

        // Clear all caches when locale changes
        let prev_locale = RwSignal::new(locale.get_untracked());
        Effect::new(move || {
            let current = locale.get();
            let prev = prev_locale.get_untracked();
            if current != prev {
                prev_locale.set(current);
                class_cache.clear();
                race_cache.clear();
                background_cache.clear();
                spell_list_cache.clear();
            }
        });

        Self {
            locale,
            class_index,
            effects_index,
            class_cache,
            race_cache,
            background_cache,
            spell_list_cache,
        }
    }

    // ---- DefinitionStore accessors ----

    pub fn classes(&self) -> ClassDefs {
        ClassDefs(*self)
    }

    pub fn races(&self) -> RaceDefs {
        RaceDefs(*self)
    }

    pub fn backgrounds(&self) -> BackgroundDefs {
        BackgroundDefs(*self)
    }

    // ---- Internal helpers ----

    pub(super) fn localized_url(&self, path: &str) -> String {
        let locale = self.locale.get_untracked();
        format!("{BASE_URL}/{locale}/{path}")
    }

    /// Resolve an index URL by looking up `name` in a specific index field.
    /// When `tracked` is true, the read subscribes to reactive updates.
    fn resolve_index_url<T>(
        &self,
        name: &str,
        extractor: impl FnOnce(&Index) -> &BTreeMap<Box<str>, T>,
        tracked: bool,
    ) -> Option<String>
    where
        T: HasUrl,
    {
        let guard = if tracked {
            self.class_index.read()
        } else {
            self.class_index.read_untracked()
        };
        let index = guard.as_ref()?.as_ref().ok()?;
        let entry = extractor(index).get(name)?;
        Some(self.localized_url(entry.url()))
    }

    /// Access a specific index field, calling `f` with the entries map.
    fn with_index_field<T, R>(
        &self,
        extractor: impl FnOnce(&Index) -> &BTreeMap<Box<str>, T>,
        empty: &BTreeMap<Box<str>, T>,
        f: impl FnOnce(&BTreeMap<Box<str>, T>) -> R,
    ) -> R {
        let guard = self.class_index.read();
        let entries = guard.as_ref().and_then(|r| r.as_ref().ok()).map(extractor);
        f(entries.unwrap_or(empty))
    }

    pub fn track_spell_cache(&self) {
        self.spell_list_cache.track();
    }

    // ---- Effects ----

    pub fn with_effects_index<R>(
        &self,
        f: impl FnOnce(&BTreeMap<Box<str>, ActiveEffect>) -> R,
    ) -> R {
        static EMPTY: BTreeMap<Box<str>, ActiveEffect> = BTreeMap::new();
        let guard = self.effects_index.read();
        let index: Option<&EffectsIndex> = guard.as_ref().and_then(|r| r.as_ref().ok());
        f(index.map_or(&EMPTY, |idx| &idx.0))
    }

    // ---- Spells ----

    pub fn fetch_spell_list(&self, path: &str) {
        let url = self.localized_url(path);
        self.spell_list_cache.fetch(path, url, "spell list");
    }

    pub fn with_spell_list<R>(
        &self,
        list: &SpellList,
        f: impl FnOnce(&BTreeMap<Box<str>, SpellDefinition>) -> R,
    ) -> R {
        match list {
            SpellList::Inline(spells) => f(spells),
            SpellList::Ref { from } => {
                self.fetch_spell_list(from);
                let cache = self.spell_list_cache.read_untracked();
                f(cache.get(from.as_str()).map_or(&EMPTY_SPELL_MAP, |v| &v.0))
            }
        }
    }

    pub fn fetch_spell_list_tracked(&self, path: &str) {
        let url = {
            let guard = self.class_index.read();
            let index = match guard.as_ref().and_then(|r| r.as_ref().ok()) {
                Some(idx) => idx,
                None => return,
            };
            match index
                .spells
                .values()
                .find(|e| e.url == path || e.name == path)
            {
                Some(entry) => self.localized_url(&entry.url),
                None => self.localized_url(path),
            }
        };

        self.spell_list_cache.fetch(path, url, "spell list");
    }

    pub fn with_spell_list_tracked<R>(
        &self,
        path: &str,
        f: impl FnOnce(&BTreeMap<Box<str>, SpellDefinition>) -> R,
    ) -> Option<R> {
        self.spell_list_cache.read().get(path).map(|v| f(&v.0))
    }

    // ---- Feature lookup (delegates to resolve module) ----

    pub fn with_feature<R>(
        &self,
        identity: &CharacterIdentity,
        feature_name: &str,
        f: impl FnOnce(&FeatureDefinition) -> R,
    ) -> Option<R> {
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();
        resolve::find_feature(identity, feature_name, &class_cache, &bg_cache, &race_cache).map(f)
    }

    pub fn with_feature_source<R>(
        &self,
        identity: &CharacterIdentity,
        feature_name: &str,
        f: impl FnOnce(&FeatureDefinition, FeatureSource) -> R,
    ) -> Option<R> {
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();
        resolve::find_feature_with_source(
            identity,
            feature_name,
            &class_cache,
            &bg_cache,
            &race_cache,
        )
        .map(|(feat, source)| f(feat, source))
    }

    pub fn feature_class_level(
        &self,
        identity: &CharacterIdentity,
        feature_name: &str,
    ) -> Option<u32> {
        let class_cache = self.class_cache.read_untracked();
        resolve::feature_class_level(identity, feature_name, &class_cache)
    }

    // ---- Choice / Points helpers ----

    pub fn get_choice_options(
        &self,
        classes: &[ClassLevel],
        feature_name: &str,
        field_name: &str,
        character_fields: &[FeatureField],
    ) -> Vec<ChoiceOption> {
        let cache = self.class_cache.read_untracked();
        for cl in classes {
            if let Some(def) = cache.get(cl.class.as_str())
                && let Some(feat) = def.find_feature(feature_name, cl.subclass.as_deref())
                && let Some(field_def) = feat.fields.get(field_name)
            {
                return field_def.resolve_choice_options(character_fields, cl.level);
            }
        }
        Vec::new()
    }

    pub fn get_choice_cost_label(
        &self,
        classes: &[ClassLevel],
        feature_name: &str,
        field_name: &str,
    ) -> Option<String> {
        let cache = self.class_cache.read_untracked();
        for cl in classes {
            if let Some(def) = cache.get(cl.class.as_str())
                && let Some(feat) = def.find_feature(feature_name, cl.subclass.as_deref())
                && let Some(fd) = feat.fields.get(field_name)
                && let FieldKind::Choice { cost, .. } = &fd.kind
                && let Some(cost_name) = cost
            {
                let short = def
                    .features(cl.subclass.as_deref())
                    .flat_map(|f| f.fields.values())
                    .find(|f| f.name == *cost_name)
                    .and_then(|f| {
                        if let FieldKind::Points { short, .. } = &f.kind {
                            short.as_deref()
                        } else {
                            None
                        }
                    })
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| cost_name.clone());
                return Some(short);
            }
        }
        None
    }

    pub fn get_points_short(
        &self,
        identity: &CharacterIdentity,
        field_name: &str,
    ) -> Option<String> {
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();

        for cl in &identity.classes {
            if let Some(def) = class_cache.get(cl.class.as_str()) {
                for feat in def.features(cl.subclass.as_deref()) {
                    if let Some(fd) = feat.fields.get(field_name)
                        && let FieldKind::Points { short: Some(s), .. } = &fd.kind
                    {
                        return Some(s.clone());
                    }
                }
            }
        }

        if let Some(bg) = bg_cache.get(identity.background.as_str()) {
            for feat in bg.features.values() {
                if let Some(fd) = feat.fields.get(field_name)
                    && let FieldKind::Points { short: Some(s), .. } = &fd.kind
                {
                    return Some(s.clone());
                }
            }
        }

        if let Some(race) = race_cache.get(identity.race.as_str()) {
            for feat in race.features.values() {
                if let Some(fd) = feat.fields.get(field_name)
                    && let FieldKind::Points { short: Some(s), .. } = &fd.kind
                {
                    return Some(s.clone());
                }
            }
        }

        None
    }

    // ---- Fill / Clear ----

    pub fn fill_from_registry(&self, character: &mut Character) {
        // Tracked read of the index so the calling Effect re-runs when
        // the index arrives.
        let index_guard = self.class_index.read();
        let index = index_guard.as_ref().and_then(|r| r.as_ref().ok());

        // Trigger fetches for any missing definitions
        if let Some(idx) = index {
            for cl in &character.identity.classes {
                if !cl.class.is_empty()
                    && let Some(entry) = idx.classes.get(cl.class.as_str())
                {
                    let url = self.localized_url(&entry.url);
                    self.class_cache.fetch(&cl.class, url, "class definition");
                }
            }
            if !character.identity.race.is_empty()
                && let Some(entry) = idx.races.get(character.identity.race.as_str())
            {
                let url = self.localized_url(&entry.url);
                self.race_cache
                    .fetch(&character.identity.race, url, "race definition");
            }
            if !character.identity.background.is_empty()
                && let Some(entry) = idx.backgrounds.get(character.identity.background.as_str())
            {
                let url = self.localized_url(&entry.url);
                self.background_cache.fetch(
                    &character.identity.background,
                    url,
                    "background definition",
                );
            }
        }

        // Trigger spell list fetches
        self.trigger_spell_list_fetches(character);

        let class_cache = self.class_cache.read();
        let bg_cache = self.background_cache.read();
        let race_cache = self.race_cache.read();
        let spell_list_cache = self.spell_list_cache.read();

        labels::sync_labels(
            character,
            &class_cache,
            &bg_cache,
            &race_cache,
            &spell_list_cache,
            // Fill: set label if None
            |target, source| {
                if target.is_none() {
                    *target = source.map(String::from);
                }
            },
            // Fill: set description if empty
            |target, source| {
                if target.is_empty() && !source.is_empty() {
                    source.clone_into(target);
                }
            },
            // Fill: set cost and free_uses from definition
            |spell, spell_def, free_uses_max| {
                if let Some(def) = spell_def {
                    spell.cost = def.cost;
                    if def.cost > 0 && free_uses_max > 0 {
                        match &mut spell.free_uses {
                            Some(fu) => fu.max = free_uses_max,
                            None => {
                                spell.free_uses = Some(FreeUses {
                                    used: 0,
                                    max: free_uses_max,
                                });
                            }
                        }
                    }
                }
            },
        );
    }

    pub fn clear_from_registry(&self, character: &mut Character) {
        let class_cache = self.class_cache.read_untracked();
        let bg_cache = self.background_cache.read_untracked();
        let race_cache = self.race_cache.read_untracked();
        let spell_list_cache = self.spell_list_cache.read_untracked();

        labels::sync_labels(
            character,
            &class_cache,
            &bg_cache,
            &race_cache,
            &spell_list_cache,
            // Clear: clear label if matches
            |target, source| {
                if target.as_deref() == source {
                    *target = None;
                }
            },
            // Clear: clear description if matches
            |target, source| {
                if *target == source {
                    target.clear();
                }
            },
            // Clear: zero cost and remove free_uses
            |spell, _, _| {
                spell.cost = 0;
                spell.free_uses = None;
            },
        );
    }
}

// ---- Trait helpers for index entries ----

trait HasUrl {
    fn url(&self) -> &str;
}

trait HasLabel {
    fn label(&self) -> &str;
}

macro_rules! impl_index_entry_traits {
    ($ty:ty) => {
        impl HasUrl for $ty {
            fn url(&self) -> &str {
                &self.url
            }
        }

        impl HasLabel for $ty {
            fn label(&self) -> &str {
                self.label()
            }
        }
    };
}

impl_index_entry_traits!(ClassIndexEntry);
impl_index_entry_traits!(RaceIndexEntry);
impl_index_entry_traits!(BackgroundIndexEntry);
impl_index_entry_traits!(SpellIndexEntry);

fn label_by_name<T: HasLabel>(entries: &BTreeMap<Box<str>, T>, name: &str) -> String {
    entries
        .get(name)
        .map(|e| e.label().to_string())
        .unwrap_or_default()
}

// Empty maps for when index isn't loaded yet
static EMPTY_SPELL_MAP: BTreeMap<Box<str>, SpellDefinition> = BTreeMap::new();
