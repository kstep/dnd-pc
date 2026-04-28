use std::{borrow::Borrow, collections::BTreeMap};

use leptos::{prelude::*, reactive::wrappers::read::ArcSignal};
use leptos_fluent::tr;

use crate::{
    BASE_URL,
    demap::Named,
    model::{
        Character, CharacterIdentity, ClassLevel, EffectTemplate, EffectsIndex, FeatureField,
        FeatureSource, FreeUses,
    },
    rules::{
        background::BackgroundDefinition,
        cache::{DefinitionStore, FetchCache, LocalizedCache},
        class::ClassDefinition,
        feature::{ChoiceOption, FeatureDefinition, FeaturesIndex, FieldKind},
        index::{
            BackgroundIndexEntry, ClassIndexEntry, Index, IndexEntry, SpeciesIndexEntry,
            SpellIndexEntry,
        },
        labels,
        locale::{
            EffectsLocaleMap, IndexLocaleMap, LocaleMap, LocaleText, LocalizedText, SpellsLocaleMap,
        },
        resolve,
        species::SpeciesDefinition,
        spells::{EMPTY_SPELL_INDEX, SpellDefinition, SpellsIndex, SpellsList},
        utils::fetch_json,
    },
};

// ---- DefinitionStore newtype wrappers ----

pub struct ClassDefs(RulesRegistry);
pub struct SpeciesDefs(RulesRegistry);
pub struct BackgroundDefs(RulesRegistry);

macro_rules! impl_definition_store {
    ($wrapper:ty, $def:ty, $cache:ident, $index_field:ident, $label:expr) => {
        impl DefinitionStore for $wrapper {
            type Definition = $def;

            fn cache(&self) -> LocalizedCache<$def> {
                self.0.$cache
            }

            fn data_url(&self, name: &str) -> Option<String> {
                self.0
                    .resolve_url(name, |idx| &idx.$index_field, true, RulesRegistry::data_url)
            }

            fn locale_url(&self, name: &str) -> Option<String> {
                self.0.resolve_url(
                    name,
                    |idx| &idx.$index_field,
                    true,
                    |p| self.0.localized_url(p),
                )
            }

            fn data_url_untracked(&self, name: &str) -> Option<String> {
                self.0.resolve_url(
                    name,
                    |idx| &idx.$index_field,
                    false,
                    RulesRegistry::data_url,
                )
            }

            fn locale_url_untracked(&self, name: &str) -> Option<String> {
                self.0.resolve_url(
                    name,
                    |idx| &idx.$index_field,
                    false,
                    |p| self.0.localized_url(p),
                )
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
    SpeciesDefs,
    SpeciesDefinition,
    species_cache,
    species,
    "species definition"
);
impl_definition_store!(
    BackgroundDefs,
    BackgroundDefinition,
    background_cache,
    backgrounds,
    "background definition"
);

macro_rules! index_accessors {
    ($($method:ident, $field:ident, $entry:ty);+ $(;)?) => {
        $(
            pub fn $method<R>(
                &self,
                f: impl FnOnce(&BTreeMap<Box<str>, $entry>) -> R,
            ) -> R {
                static EMPTY: BTreeMap<Box<str>, $entry> = BTreeMap::new();
                self.with_index_field(|idx| &idx.$field, &EMPTY, f)
            }
        )+
    };
}

// ---- RulesRegistry ----

#[derive(Clone, Copy)]
pub struct RulesRegistry {
    locale: Signal<String>,
    class_index: LocalizedIndex<Index, IndexLocaleMap>,
    pub(super) class_cache: LocalizedCache<ClassDefinition>,
    pub(super) species_cache: LocalizedCache<SpeciesDefinition>,
    pub(super) background_cache: LocalizedCache<BackgroundDefinition>,
    /// Per-class curated name lists from `public/data/spells/*.json` —
    /// each value is a flat `Vec<String>`, fetched on demand and locale-less.
    spell_names_cache: FetchCache<Vec<String>>,
    /// Global spells index loaded once from `public/data/spells.json`.
    /// Locale overlay re-fetched on language change.
    pub(super) spells_index: LocalizedIndex<SpellsIndex, SpellsLocaleMap>,
    effects_index: LocalizedIndex<EffectsIndex, EffectsLocaleMap>,
    pub(super) features_index: LocalizedIndex<FeaturesIndex, LocaleMap>,
}

impl RulesRegistry {
    // ---- Index-based methods (stay on RulesRegistry) ----

    index_accessors! {
        with_class_entries,      classes,      ClassIndexEntry;
        with_species_entries,    species,      SpeciesIndexEntry;
        with_background_entries, backgrounds,  BackgroundIndexEntry;
        with_spell_entries,      spells,       SpellIndexEntry;
    }

    pub fn canonical_class_name(&self, query: &str) -> Option<String> {
        self.with_class_entries(|entries| canonical_name(entries, query))
    }

    pub fn canonical_species_name(&self, query: &str) -> Option<String> {
        self.with_species_entries(|entries| canonical_name(entries, query))
    }

    pub fn canonical_background_name(&self, query: &str) -> Option<String> {
        self.with_background_entries(|entries| canonical_name(entries, query))
    }

    /// Resolve a subclass name within a class. Requires the class definition
    /// to be loaded.
    pub fn canonical_subclass_name(&self, class_name: &str, query: &str) -> Option<String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return None;
        }
        self.classes()
            .with(class_name, |def| {
                if let Some(entry) = def.subclasses.get(trimmed) {
                    return Some(entry.name.to_string());
                }
                def.subclasses
                    .values()
                    .find(|sub| sub.name.eq_ignore_ascii_case(trimmed))
                    .map(|sub| sub.name.to_string())
            })
            .flatten()
    }

    /// Checks whether `character` can multiclass into `class_name`.
    /// All existing classes and the candidate class must meet their
    /// prerequisites.
    pub fn can_multiclass(&self, character: &Character, class_name: &str) -> bool {
        self.with_class_entries(|entries| {
            // Every existing class must meet its prerequisites.
            let existing_ok = character.identity.classes.iter().all(|cl| {
                cl.class.is_empty()
                    || entries
                        .get(cl.class.as_str())
                        .is_none_or(|entry| entry.meets_prerequisites(character))
            });
            // The candidate class must also meet its prerequisites.
            existing_ok
                && entries
                    .get(class_name)
                    .is_none_or(|entry| entry.meets_prerequisites(character))
        })
    }

    pub fn is_loading(&self) -> bool {
        self.class_cache.is_pending()
            || self.species_cache.is_pending()
            || self.background_cache.is_pending()
            || self.spell_names_cache.is_pending()
            || self.class_index.is_pending()
            || self.features_index.is_pending()
            || self.spells_index.is_pending()
            || self.effects_index.is_pending()
    }

    pub fn new(i18n: leptos_fluent::I18n) -> Self {
        let locale = Signal::derive(move || i18n.language.get().id.to_string());

        let class_index = LocalizedIndex::<Index, IndexLocaleMap>::new(locale, "index.json");
        let effects_index =
            LocalizedIndex::<EffectsIndex, EffectsLocaleMap>::new(locale, "effects.json");
        let features_index =
            LocalizedIndex::<FeaturesIndex, LocaleMap>::new(locale, "features.json");
        let spells_index =
            LocalizedIndex::<SpellsIndex, SpellsLocaleMap>::new(locale, "spells.json");

        let class_cache = LocalizedCache::new();
        let species_cache = LocalizedCache::new();
        let background_cache = LocalizedCache::new();
        let spell_names_cache = FetchCache::new();

        // On locale change, drop locale overlays so subsequent lookups (driven
        // by ensure_definitions_fetched re-running) refetch fresh overlays.
        // Data caches are locale-independent and survive.
        Effect::new(move || {
            locale.track();
            class_cache.clear_locale();
            species_cache.clear_locale();
            background_cache.clear_locale();
        });

        Self {
            locale,
            class_index,
            effects_index,
            features_index,
            spells_index,
            class_cache,
            species_cache,
            background_cache,
            spell_names_cache,
        }
    }

    /// Translate a feature source into a display string using cached
    /// definition labels (locale-aware).
    pub fn source_label(&self, source: &FeatureSource, i18n: leptos_fluent::I18n) -> String {
        match source {
            FeatureSource::Class(name, level) => {
                let prefix = tr!(i18n, "source-class");
                let label = self
                    .classes()
                    .lookup(name, |loc| loc.label().to_string())
                    .unwrap_or_else(|| name.to_string());
                format!("{prefix}: {label} ({level})")
            }
            FeatureSource::Subclass(class_name, subclass_name, level) => {
                let prefix = tr!(i18n, "source-subclass");
                let (class_label, subclass_label) = self
                    .classes()
                    .lookup(class_name, |loc| {
                        let class_label = loc.label().to_string();
                        let subclass_label = loc
                            .subclass(subclass_name)
                            .map(|sub| sub.label().to_string())
                            .unwrap_or_else(|| subclass_name.to_string());
                        (class_label, subclass_label)
                    })
                    .unwrap_or_else(|| (class_name.to_string(), subclass_name.to_string()));
                format!("{prefix}: {class_label} — {subclass_label} ({level})")
            }
            FeatureSource::Species(name) => {
                let prefix = tr!(i18n, "source-species");
                let label = self
                    .species()
                    .lookup(name, |loc| loc.label().to_string())
                    .unwrap_or_else(|| name.to_string());
                format!("{prefix}: {label}")
            }
            FeatureSource::Background(name) => {
                let prefix = tr!(i18n, "source-background");
                let label = self
                    .backgrounds()
                    .lookup(name, |loc| loc.label().to_string())
                    .unwrap_or_else(|| name.to_string());
                format!("{prefix}: {label}")
            }
            FeatureSource::User(level) => {
                let prefix = tr!(i18n, "source-user");
                format!("{prefix} ({level})")
            }
        }
    }

    // ---- DefinitionStore accessors ----

    pub fn classes(&self) -> ClassDefs {
        ClassDefs(*self)
    }

    pub fn species(&self) -> SpeciesDefs {
        SpeciesDefs(*self)
    }

    pub fn backgrounds(&self) -> BackgroundDefs {
        BackgroundDefs(*self)
    }

    // ---- Internal helpers ----

    fn data_url(path: &str) -> String {
        format!("{BASE_URL}/data/{path}")
    }

    pub(super) fn localized_url(&self, path: &str) -> String {
        let locale = self.locale.get_untracked();
        format!("{BASE_URL}/{locale}/{path}")
    }

    /// Look up a name in the index and apply `make_url` to the entry's path.
    fn resolve_url<T>(
        &self,
        name: &str,
        extractor: impl FnOnce(&Index) -> &BTreeMap<Box<str>, T>,
        tracked: bool,
        make_url: impl FnOnce(&str) -> String,
    ) -> Option<String>
    where
        T: HasUrl,
    {
        let resolver = |index: Option<&Index>| -> Option<String> {
            let entry = extractor(index?).get(name)?;
            Some(make_url(entry.url()))
        };
        if tracked {
            self.class_index.with_data(resolver)
        } else {
            self.class_index.with_data_untracked(resolver)
        }
    }

    /// Access a specific index field, calling `f` with the entries map.
    fn with_index_field<T, R>(
        &self,
        extractor: impl FnOnce(&Index) -> &BTreeMap<Box<str>, T>,
        empty: &BTreeMap<Box<str>, T>,
        f: impl FnOnce(&BTreeMap<Box<str>, T>) -> R,
    ) -> R {
        self.class_index.with_data(|index| {
            let entries = index.map(extractor);
            f(entries.unwrap_or(empty))
        })
    }

    pub fn track_spell_cache(&self) {
        self.spell_names_cache.track();
        self.spells_index.track();
    }

    /// Access the spells index wrapper (data + locale) directly.
    pub fn spells(&self) -> LocalizedIndex<SpellsIndex, SpellsLocaleMap> {
        self.spells_index
    }

    /// Access the features index wrapper (data + locale) directly.
    pub fn features(&self) -> LocalizedIndex<FeaturesIndex, LocaleMap> {
        self.features_index
    }

    /// Access the shared index wrapper (class/species/background/spell
    /// entries + their locale overlay) directly.
    pub fn index(&self) -> LocalizedIndex<Index, IndexLocaleMap> {
        self.class_index
    }

    // ---- Effects ----

    /// Access the effects catalog wrapper (data + locale) directly.
    pub fn effects(&self) -> LocalizedIndex<EffectsIndex, EffectsLocaleMap> {
        self.effects_index
    }

    pub fn with_effects_index<R>(
        &self,
        f: impl FnOnce(&BTreeMap<Box<str>, EffectTemplate>) -> R,
    ) -> R {
        static EMPTY: BTreeMap<Box<str>, EffectTemplate> = BTreeMap::new();
        self.effects_index
            .with_data(|index| f(index.map_or(&EMPTY, |idx| &idx.0)))
    }

    // ---- Features index ----

    pub fn with_features_index<R>(
        &self,
        f: impl FnOnce(&BTreeMap<Box<str>, FeatureDefinition>) -> R,
    ) -> R {
        static EMPTY: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        let guard = self.features_index.data.read();
        let index: Option<&FeaturesIndex> = guard.as_ref().and_then(|r| r.as_ref().ok());
        f(index.map_or(&EMPTY, |idx| &idx.0))
    }

    pub fn with_features_index_untracked<R>(
        &self,
        f: impl FnOnce(&BTreeMap<Box<str>, FeatureDefinition>) -> R,
    ) -> R {
        static EMPTY: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        let guard = self.features_index.data.read_untracked();
        let index: Option<&FeaturesIndex> = guard.as_ref().and_then(|r| r.as_ref().ok());
        f(index.map_or(&EMPTY, |idx| &idx.0))
    }

    /// Nested helper for the apply pipeline — both indexes available in one
    /// closure. Saves the verbose double-nesting pattern at every call site.
    pub fn with_apply_indexes<R>(
        &self,
        f: impl FnOnce(
            &BTreeMap<Box<str>, FeatureDefinition>,
            &BTreeMap<Box<str>, SpellDefinition>,
        ) -> R,
    ) -> R {
        self.with_features_index_untracked(|feat_index| {
            self.with_spells_index_untracked(|spell_index| f(feat_index, spell_index))
        })
    }

    // ---- Spells index ----

    pub fn with_spells_index<R>(
        &self,
        f: impl FnOnce(&BTreeMap<Box<str>, SpellDefinition>) -> R,
    ) -> R {
        let guard = self.spells_index.data.read();
        let index: Option<&SpellsIndex> = guard.as_ref().and_then(|r| r.as_ref().ok());
        f(index.map_or(&EMPTY_SPELL_INDEX, |idx| &idx.0))
    }

    pub fn with_spells_index_untracked<R>(
        &self,
        f: impl FnOnce(&BTreeMap<Box<str>, SpellDefinition>) -> R,
    ) -> R {
        let guard = self.spells_index.data.read_untracked();
        let index: Option<&SpellsIndex> = guard.as_ref().and_then(|r| r.as_ref().ok());
        f(index.map_or(&EMPTY_SPELL_INDEX, |idx| &idx.0))
    }

    /// Fetch the per-class name list (e.g. `"spells/wizard.json"`) into the
    /// names cache. The list is locale-less — just `["Acid Splash", ...]`.
    pub fn fetch_spell_list_untracked(&self, path: &str) {
        self.spell_names_cache
            .fetch(path, Self::data_url(path), "spell list");
    }

    /// Tracked variant — re-runs the calling Effect when the per-class file
    /// arrives. Resolves `path` through the index when it matches a list name
    /// rather than a direct file path.
    pub fn fetch_spell_list(&self, path: &str) {
        let resolved_path = self.class_index.with_data(|index| {
            index?
                .spells
                .values()
                .find(|entry| &*entry.url == path || &*entry.name == path)
                .map(|entry| entry.url.to_string())
        });
        let resolved_path = resolved_path.unwrap_or_else(|| path.to_string());
        self.spell_names_cache
            .fetch(&resolved_path, Self::data_url(&resolved_path), "spell list");
    }

    /// Iterate (name, definition) pairs implied by a feature's `SpellsList`.
    /// `Items` enumerates entry names; `Ref { from }` reads the cached
    /// per-class name list (triggering a fetch on miss) and resolves each
    /// name against the global spells index. Both signal guards are held
    /// for the duration of the closure; no allocation.
    pub fn with_spell_list_untracked<R>(
        &self,
        list: &SpellsList,
        f: impl FnOnce(&mut dyn Iterator<Item = LocalizedText<'_, SpellDefinition, LocaleText>>) -> R,
    ) -> R {
        let data_guard = self.spells_index.data.read_untracked();
        let locale_guard = self.spells_index.locale.read_untracked();
        let Some(index) = data_guard.as_ref().and_then(|r| r.as_ref().ok()) else {
            return f(&mut std::iter::empty());
        };
        let locale: Option<&SpellsLocaleMap> = locale_guard.as_ref().and_then(|opt| opt.as_ref());
        match list {
            SpellsList::Inline(entries) => {
                let mut iter = entries.iter().filter_map(|entry| {
                    let name = &*entry.name;
                    index.0.get(name).map(|def| LocalizedText {
                        data: def,
                        locale: locale.and_then(|m| m.get(name)),
                    })
                });
                f(&mut iter)
            }
            SpellsList::Ref { from } => {
                self.fetch_spell_list(from);
                let names_guard = self.spell_names_cache.read_untracked();
                match names_guard.get(from.as_str()) {
                    Some(names) => {
                        let mut iter = names.iter().filter_map(|name| {
                            index.0.get(name.as_str()).map(|def| LocalizedText {
                                data: def,
                                locale: locale.and_then(|m| m.get(name.as_str())),
                            })
                        });
                        f(&mut iter)
                    }
                    None => f(&mut std::iter::empty()),
                }
            }
        }
    }

    /// Tracked variant for a `Ref { from }` path — reads with reactivity so
    /// the calling Effect re-runs when the list arrives. Returns `None` if
    /// the spells index or the per-class list haven't loaded yet.
    pub fn with_spell_list<R>(
        &self,
        path: &str,
        f: impl FnOnce(&mut dyn Iterator<Item = LocalizedText<'_, SpellDefinition, LocaleText>>) -> R,
    ) -> Option<R> {
        self.spells_index
            .with(|index, locale| {
                let names_guard = self.spell_names_cache.read();
                let names = names_guard.get(path)?;
                let mut iter = names.iter().filter_map(|name| {
                    index.0.get(name.as_str()).map(|def| LocalizedText {
                        data: def,
                        locale: locale.and_then(|m| m.get(name.as_str())),
                    })
                });
                Some(f(&mut iter))
            })
            .flatten()
    }

    // ---- Feature lookup (delegates to resolve module) ----

    pub fn with_feature<R>(
        &self,
        feature_name: &str,
        f: impl FnOnce(&FeatureDefinition) -> R,
    ) -> Option<R> {
        self.with_features_index_untracked(|features_index| features_index.get(feature_name).map(f))
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
        self.with_features_index_untracked(|features_index| {
            if let Some(feat) = features_index.get(feature_name)
                && let Some(field_def) = feat.fields.get(field_name)
            {
                let level = self.feature_class_level_for(classes, feature_name);
                return field_def.resolve_choice_options(character_fields, level);
            }
            Vec::new()
        })
    }

    pub fn get_choice_cost_label(&self, feature_name: &str, field_name: &str) -> Option<String> {
        self.with_features_index_untracked(|features_index| {
            let feat = features_index.get(feature_name)?;
            let fd = feat.fields.get(field_name)?;
            let FieldKind::Choice { cost, .. } = &fd.kind else {
                return None;
            };
            let cost_name = cost.as_ref()?;
            Some(crate::model::short_name(cost_name))
        })
    }

    fn feature_class_level_for(&self, classes: &[ClassLevel], feature_name: &str) -> u32 {
        let class_cache = self.class_cache.read_untracked();
        resolve::feature_class_level_from_classes(classes, feature_name, &class_cache).unwrap_or(0)
    }

    // ---- Fill / Clear ----

    /// Trigger fetches for any missing definitions. Reads the index with a
    /// tracked read so the calling Effect re-runs when the index arrives.
    /// Does NOT read caches or update the store — cheap to re-run.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.ensure_definitions_fetched", skip_all)
    )]
    pub fn ensure_definitions_fetched(&self, character: &Character) {
        for class_level in &character.identity.classes {
            if !class_level.class.is_empty() {
                self.classes().fetch(&class_level.class);
            }
        }
        if !character.identity.species.is_empty() {
            self.species().fetch(&character.identity.species);
        }
        if !character.identity.background.is_empty() {
            self.backgrounds().fetch(&character.identity.background);
        }
        self.trigger_spell_list_fetches(character);
    }

    /// Fill labels and descriptions from cached definitions. Reads caches
    /// with tracked reads so the calling Effect re-runs when definitions
    /// arrive or locale changes.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.fill_from_registry", skip_all)
    )]
    pub fn fill_from_registry(&self, character: &mut Character) {
        // Track all sources so the calling Effect re-runs on locale switch
        // or arrival of any resource.
        self.features_index.track();
        self.spells_index.track();
        self.class_cache.locale.track();

        labels::sync_labels(
            character,
            self.class_cache.locale,
            self.features_index,
            self.spells_index,
            // Fill: always overwrite label from definition (supports locale
            // switching without a separate clear_all_labels step).
            |target, source| {
                *target = source.map(String::from);
            },
            // Fill: always overwrite description from definition.
            |target, source| {
                if !source.is_empty() {
                    source.clone_into(target);
                }
            },
            // Fill: copy cost from the feature's spell entry; create
            // free_uses tracker if the spell has cost and a FreeUses
            // pool exists on the feature.
            |spell, extras| {
                spell.cost = extras.cost;
                if extras.cost > 0 && extras.free_uses_max > 0 {
                    match &mut spell.free_uses {
                        Some(fu) => fu.max = extras.free_uses_max,
                        None => {
                            spell.free_uses = Some(FreeUses {
                                used: 0,
                                max: extras.free_uses_max,
                            });
                        }
                    }
                }
            },
        );
    }

    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.clear_from_registry", skip_all)
    )]
    pub fn clear_from_registry(&self, character: &mut Character) {
        labels::sync_labels(
            character,
            self.class_cache.locale,
            self.features_index,
            self.spells_index,
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
            |spell, _| {
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

macro_rules! impl_index_entry_traits {
    ($ty:ty) => {
        impl HasUrl for $ty {
            fn url(&self) -> &str {
                &self.url
            }
        }
    };
}

impl_index_entry_traits!(ClassIndexEntry);
impl_index_entry_traits!(SpeciesIndexEntry);
impl_index_entry_traits!(BackgroundIndexEntry);
impl_index_entry_traits!(SpellIndexEntry);

/// Pairs a structural data resource with its current-locale overlay
/// resource. Replaces `make_localized_index`'s mutate-in-place strategy
/// with two independent `LocalResource`s — zero deep clones on locale
/// switch (only the locale map is re-fetched and dropped).
pub struct LocalizedIndex<T: Send + Sync + 'static, L: Send + Sync + 'static> {
    pub(super) data: LocalResource<Result<T, String>>,
    pub(super) locale: LocalResource<Option<L>>,
    /// In-flight counter for data + locale fetches; `LocalResource` keeps its
    /// stale value during refetch and exposes no `loading()` signal, so we
    /// count manually inside the fetch closures.
    in_flight: RwSignal<u32>,
    /// `true` while any fetch (data or locale) is in flight, including
    /// refetches triggered by locale switch. Tracked.
    pub loading: Signal<bool>,
}

impl<T, L> Clone for LocalizedIndex<T, L>
where
    T: Send + Sync + 'static,
    L: Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, L> Copy for LocalizedIndex<T, L>
where
    T: Send + Sync + 'static,
    L: Send + Sync + 'static,
{
}

impl<T, L> std::ops::Deref for LocalizedIndex<T, L>
where
    T: Send + Sync + 'static,
    L: Send + Sync + 'static,
{
    type Target = LocalResource<Result<T, String>>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T, L> LocalizedIndex<T, L>
where
    T: Clone + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    L: Clone + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
{
    pub fn new(locale: Signal<String>, file_name: &'static str) -> Self {
        let in_flight = RwSignal::new(0u32);

        // Both resources read locale UNTRACKED — refetch is driven explicitly
        // by the Effect below so we have a single point that toggles the
        // in-flight counter via `refetch()`.
        let data = LocalResource::new(move || {
            let url = format!("{BASE_URL}/data/{file_name}");
            async move { fetch_json::<T>(&url).await }
        });
        let locale_resource = LocalResource::new(move || {
            let lang = locale.get_untracked();
            let url = format!("{BASE_URL}/{lang}/{file_name}");
            async move { fetch_json::<L>(&url).await.ok() }
        });

        let loading = Signal::derive(move || in_flight.get() > 0);

        let this = Self {
            data,
            locale: locale_resource,
            in_flight,
            loading,
        };

        // Count the initial loads so `loading` is true until first fetch lands.
        this.observe_initial();

        // Auto-refetch on locale switch.
        Effect::new(move |prev: Option<()>| {
            locale.track();
            if prev.is_some() {
                this.refetch();
            }
        });

        this
    }

    /// Spawn observers for the initial data + locale fetches; each await
    /// completion decrements `in_flight`.
    fn observe_initial(&self) {
        let in_flight = self.in_flight;
        in_flight.update(|count| *count += 2);
        let data = self.data;
        let locale = self.locale;
        leptos::task::spawn_local(async move {
            let _ = data.into_future().await;
            in_flight.update(|count| *count = count.saturating_sub(1));
        });
        leptos::task::spawn_local(async move {
            let _ = locale.into_future().await;
            in_flight.update(|count| *count = count.saturating_sub(1));
        });
    }

    /// Manually re-fetch the locale overlay (data is locale-independent).
    /// Bumps `loading` for the duration of the fetch.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "localized_index.refetch", skip_all)
    )]
    pub fn refetch(&self) {
        let in_flight = self.in_flight;
        let locale_res = self.locale;
        in_flight.update(|count| *count += 1);
        locale_res.refetch();
        leptos::task::spawn_local(async move {
            let _ = locale_res.into_future().await;
            in_flight.update(|count| *count = count.saturating_sub(1));
        });
    }

    /// Tracked alias of `with` returning the closure result via `Option`.
    pub fn track(&self) {
        self.data.track();
        self.locale.track();
    }

    /// `true` if any underlying fetch (data or locale) is in flight, including
    /// refetches triggered by locale switch. Shorthand for
    /// `self.loading.get()`.
    pub fn is_pending(&self) -> bool {
        self.loading.get()
    }

    /// Tracked: re-runs the calling Effect on data arrival or locale switch.
    pub fn with<R>(&self, f: impl FnOnce(&T, Option<&L>) -> R) -> Option<R> {
        let data_guard = self.data.read();
        let data = data_guard.as_ref()?.as_ref().ok()?;
        let locale_guard = self.locale.read();
        let locale = locale_guard.as_ref().and_then(|opt| opt.as_ref());
        Some(f(data, locale))
    }

    /// Untracked variant for non-reactive callers (apply pipeline, etc.).
    pub fn with_untracked<R>(&self, f: impl FnOnce(&T, Option<&L>) -> R) -> Option<R> {
        let data_guard = self.data.read_untracked();
        let data = data_guard.as_ref()?.as_ref().ok()?;
        let locale_guard = self.locale.read_untracked();
        let locale = locale_guard.as_ref().and_then(|opt| opt.as_ref());
        Some(f(data, locale))
    }

    /// Tracked read of the data resource only (no locale). Closure always
    /// runs; receives `None` if data hasn't loaded yet.
    pub fn with_data<R>(&self, f: impl FnOnce(Option<&T>) -> R) -> R {
        let guard = self.data.read();
        let value = guard.as_ref().and_then(|r| r.as_ref().ok());
        f(value)
    }

    /// Untracked variant of `with_data`.
    pub fn with_data_untracked<R>(&self, f: impl FnOnce(Option<&T>) -> R) -> R {
        let guard = self.data.read_untracked();
        let value = guard.as_ref().and_then(|r| r.as_ref().ok());
        f(value)
    }

    /// Tracked read of the locale resource only.
    pub fn with_locale<R>(&self, f: impl FnOnce(Option<&L>) -> R) -> R {
        let guard = self.locale.read();
        let value = guard.as_ref().and_then(|opt| opt.as_ref());
        f(value)
    }
}

/// Reactive label/description signals for the shared `Index` resource
/// (class/species/background/spell entries under prefixed keys).
impl LocalizedIndex<Index, IndexLocaleMap> {
    /// Reactive (label, description) for an entry. Composes the locale
    /// key from `entry`'s `Display` impl (`"class.wizard"`, …); falls
    /// back to the bare name when no locale entry is present.
    pub fn entry_label_desc(
        &self,
        entry: IndexEntry<'_>,
    ) -> (ArcSignal<String>, ArcSignal<String>) {
        self.label_desc(entry.to_string(), entry.name())
    }
}

/// Reactive label/description signals for any locale map keyed by a
/// `Borrow<str>` type. Subscribes to the index's locale resource only —
/// derived signals are owned by the calling scope, so callers should
/// create them where they want the lifetime (e.g. inside a `<For>`
/// child closure).
impl<T, K> LocalizedIndex<T, BTreeMap<K, LocaleText>>
where
    T: Clone + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
    K: 'static + Ord + Borrow<str> + Send + Sync,
{
    pub fn label_desc(
        &self,
        key: impl Into<String>,
        fallback: impl Into<String>,
    ) -> (ArcSignal<String>, ArcSignal<String>) {
        let locale = self.locale;
        let key: String = key.into();
        let fallback: String = fallback.into();
        let label = ArcSignal::derive({
            let key = key.clone();
            let fallback = fallback.clone();
            move || {
                locale
                    .read()
                    .as_ref()
                    .and_then(|opt| opt.as_ref())
                    .and_then(|m| m.get(key.as_str()))
                    .and_then(|t| t.label.clone())
                    .unwrap_or_else(|| fallback.clone())
            }
        });
        let description = ArcSignal::derive(move || {
            locale
                .read()
                .as_ref()
                .and_then(|opt| opt.as_ref())
                .and_then(|m| m.get(key.as_str()))
                .and_then(|t| t.description.clone())
                .unwrap_or_default()
        });
        (label, description)
    }
}

/// Spells-specific helpers. The flat-key locale map (`Box<str> -> LocaleText`)
/// makes per-name lookup trivial; we expose `LocalizedText<SpellDefinition,
/// LocaleText>` so consumers reach `.label()` / `.description()` without
/// knowing the internal `BTreeMap` shape.
impl LocalizedIndex<SpellsIndex, SpellsLocaleMap> {
    /// Look up one spell by name and present it as `LocalizedText`.
    pub fn lookup<R>(
        &self,
        name: &str,
        f: impl FnOnce(LocalizedText<'_, SpellDefinition, LocaleText>) -> R,
    ) -> Option<R> {
        self.with(|index, locale| {
            let def = index.0.get(name)?;
            let loc = locale.and_then(|m| m.get(name));
            Some(f(LocalizedText {
                data: def,
                locale: loc,
            }))
        })
        .flatten()
    }

    /// Untracked variant for the apply pipeline / non-reactive callers.
    pub fn lookup_untracked<R>(
        &self,
        name: &str,
        f: impl FnOnce(LocalizedText<'_, SpellDefinition, LocaleText>) -> R,
    ) -> Option<R> {
        self.with_untracked(|index, locale| {
            let def = index.0.get(name)?;
            let loc = locale.and_then(|m| m.get(name));
            Some(f(LocalizedText {
                data: def,
                locale: loc,
            }))
        })
        .flatten()
    }

    /// Iterate the index as `LocalizedText` wrappers.
    pub fn iter<R>(
        &self,
        f: impl FnOnce(&mut dyn Iterator<Item = LocalizedText<'_, SpellDefinition, LocaleText>>) -> R,
    ) -> Option<R> {
        self.with(|index, locale| {
            let mut it = index.0.values().map(move |def| LocalizedText {
                data: def,
                locale: locale.and_then(|m| m.get(&*def.name)),
            });
            f(&mut it)
        })
    }
}

/// Features-specific helpers. Locale is the entire `LocaleMap` (nested keys
/// for fields/options); the wrapper exposes `.label()`/`.description()` for
/// the feature itself plus `.field(name)`/`field.option(name)` for nested
/// lookups.
impl LocalizedIndex<FeaturesIndex, LocaleMap> {
    pub fn lookup<R>(
        &self,
        name: &str,
        f: impl FnOnce(LocalizedText<'_, FeatureDefinition, LocaleMap>) -> R,
    ) -> Option<R> {
        self.with(|index, locale| {
            let def = index.0.get(name)?;
            Some(f(LocalizedText { data: def, locale }))
        })
        .flatten()
    }

    pub fn lookup_untracked<R>(
        &self,
        name: &str,
        f: impl FnOnce(LocalizedText<'_, FeatureDefinition, LocaleMap>) -> R,
    ) -> Option<R> {
        self.with_untracked(|index, locale| {
            let def = index.0.get(name)?;
            Some(f(LocalizedText { data: def, locale }))
        })
        .flatten()
    }
}

/// Resolve a free-form query (canonical English name or localized label, any
/// case) to the canonical English `name` of an index entry. Used to harden AI
/// responses that occasionally translate names back to the prompt language.
fn canonical_name<T: Named>(entries: &BTreeMap<Box<str>, T>, query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(entry) = entries.get(trimmed) {
        return Some(entry.name().to_string());
    }
    entries
        .values()
        .find(|entry| entry.name().eq_ignore_ascii_case(trimmed))
        .map(|entry| entry.name().to_string())
}
