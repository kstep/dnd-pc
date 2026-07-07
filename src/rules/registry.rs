use std::{borrow::Borrow, collections::BTreeMap};

use leptos::{prelude::*, reactive::wrappers::read::ArcSignal};
use leptos_fluent::tr;

use crate::{
    BASE_URL,
    demap::Named,
    expr::{BLOCK_MAIN, BinOp, Cmp, Op},
    model::{
        AttrKey, Attribute, Character, CharacterCore, CharacterIdentity, ClassLevel,
        EffectTemplate, EffectsIndex, Expr, FeatureCategory, FeatureField, FeatureSource,
        IdentitySlot, intern_box,
    },
    rules::{
        WhenCondition,
        apply::DefinitionCaches,
        background::BackgroundDefinition,
        cache::{DefinitionStore, FetchCache},
        class::ClassDefinition,
        feature::{
            Assignment, ChoiceOption, EMPTY_FEATURES_INDEX, FeatureDefinition, FeaturesIndex,
            ReplaceWith,
        },
        index::IndexEntry,
        locale::{EffectsLocaleMap, LocaleMap, LocaleText, LocalizedText, SpellsLocaleMap},
        packages::{DefsIndex, PackageManifestEntry, PackageMerge},
        resolve,
        species::SpeciesDefinition,
        spells::{EMPTY_SPELL_INDEX, SpellDefinition, SpellsIndex, SpellsList},
        utils::{fetch_json, fetch_merged_json},
    },
    vecset::VecSet,
};

// ---- DefinitionStore newtype wrappers ----

pub struct ClassDefs(RulesRegistry);
pub struct SpeciesDefs(RulesRegistry);
pub struct BackgroundDefs(RulesRegistry);

macro_rules! impl_definition_store {
    ($wrapper:ty, $def:ty, $defs_field:ident) => {
        impl DefinitionStore for $wrapper {
            type Definition = $def;

            fn index(&self) -> LocalizedIndex<DefsIndex<$def>, LocaleMap> {
                self.0.$defs_field
            }
        }
    };
}

impl_definition_store!(ClassDefs, ClassDefinition, class_defs);
impl_definition_store!(SpeciesDefs, SpeciesDefinition, species_defs);
impl_definition_store!(BackgroundDefs, BackgroundDefinition, background_defs);

// ---- FeaturesView ----

/// Zero-allocation view over the natural features index plus runtime-
/// synthesized `System(_)` entries. Natural takes precedence on collision.
#[derive(Clone, Copy)]
pub struct FeaturesView<'a> {
    natural: &'a BTreeMap<Box<str>, FeatureDefinition>,
    synth: &'a BTreeMap<Box<str>, FeatureDefinition>,
}

impl<'a> FeaturesView<'a> {
    /// Construct a view over a single map without a synth overlay.
    pub fn from_natural(natural: &'a BTreeMap<Box<str>, FeatureDefinition>) -> Self {
        Self {
            natural,
            synth: &EMPTY_FEATURES_INDEX,
        }
    }

    /// View over no features at all.
    pub fn empty() -> Self {
        Self::from_natural(&EMPTY_FEATURES_INDEX)
    }

    pub fn get(&self, name: &str) -> Option<&'a FeatureDefinition> {
        self.natural.get(name).or_else(|| self.synth.get(name))
    }

    pub fn contains_key(&self, name: &str) -> bool {
        self.natural.contains_key(name) || self.synth.contains_key(name)
    }

    pub fn values(&self) -> impl Iterator<Item = &'a FeatureDefinition> + 'a {
        let natural = self.natural;
        self.natural.values().chain(
            self.synth
                .iter()
                .filter_map(move |(name, def)| (!natural.contains_key(name)).then_some(def)),
        )
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'a Box<str>, &'a FeatureDefinition)> + 'a {
        let natural = self.natural;
        self.natural.iter().chain(
            self.synth.iter().filter_map(move |(name, def)| {
                (!natural.contains_key(name)).then_some((name, def))
            }),
        )
    }
}

// ---- RulesRegistry ----

#[derive(Clone, Copy)]
pub struct RulesRegistry {
    /// Active package set; every index fetches one file per package.
    packages: Signal<VecSet<String>>,
    /// Eager merged definitions — one whole-file index per kind.
    pub(super) class_defs: LocalizedIndex<DefsIndex<ClassDefinition>, LocaleMap>,
    species_defs: LocalizedIndex<DefsIndex<SpeciesDefinition>, LocaleMap>,
    background_defs: LocalizedIndex<DefsIndex<BackgroundDefinition>, LocaleMap>,
    /// Per-class curated name lists from `rules/{pkg}/data/spells/*.json` —
    /// each value is a flat `Vec<String>`, fetched on demand (unioned across
    /// packages) and locale-less.
    spell_names_cache: FetchCache<Vec<String>>,
    /// Package manifest (`rules/index.json`) — the source of available
    /// packages; drives the default active set and import validation.
    manifest: LocalResource<Result<Vec<PackageManifestEntry>, String>>,
    /// Global spells index merged across packages.
    /// Locale overlay re-fetched on language change.
    pub(super) spells_index: LocalizedIndex<SpellsIndex, SpellsLocaleMap>,
    effects_index: LocalizedIndex<EffectsIndex, EffectsLocaleMap>,
    pub(super) features_index: LocalizedIndex<FeaturesIndex, LocaleMap>,
    /// Auto-generated `FeatureCategory::System(_)` features — one per
    /// available class / species / background / subclass. Built reactively
    /// as the defs indexes resolve. Merged with `features_index` by
    /// `with_features_index`.
    synth_features: StoredValue<BTreeMap<Box<str>, FeatureDefinition>>,
}

impl RulesRegistry {
    /// Tracked ids of every manifest-listed package, in manifest order
    /// (base first = override priority). `None` until the manifest resolves.
    pub fn manifest_ids(&self) -> Option<Vec<String>> {
        let guard = self.manifest.read();
        guard
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(|entries| entries.iter().map(|entry| entry.id.clone()).collect())
    }

    /// Untracked read of the manifest entries. `None` until resolved.
    pub fn with_manifest_untracked<R>(
        &self,
        f: impl FnOnce(&[PackageManifestEntry]) -> R,
    ) -> Option<R> {
        let guard = self.manifest.read_untracked();
        guard
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(|entries| f(entries))
    }

    /// Tracked access to the merged class definitions map (empty until the
    /// defs index resolves).
    pub fn with_class_defs<R>(
        &self,
        f: impl FnOnce(&BTreeMap<Box<str>, ClassDefinition>) -> R,
    ) -> R {
        self.class_defs
            .with_data(|defs| f(defs.map_or(&EMPTY_CLASS_DEFS, |index| &index.0)))
    }

    /// Tracked access to the merged species definitions map.
    pub fn with_species_defs<R>(
        &self,
        f: impl FnOnce(&BTreeMap<Box<str>, SpeciesDefinition>) -> R,
    ) -> R {
        self.species_defs
            .with_data(|defs| f(defs.map_or(&EMPTY_SPECIES_DEFS, |index| &index.0)))
    }

    /// Tracked access to the merged background definitions map.
    pub fn with_background_defs<R>(
        &self,
        f: impl FnOnce(&BTreeMap<Box<str>, BackgroundDefinition>) -> R,
    ) -> R {
        self.background_defs
            .with_data(|defs| f(defs.map_or(&EMPTY_BACKGROUND_DEFS, |index| &index.0)))
    }

    /// Reactive `(label, description)` for a reference entry, dispatched to
    /// the owning defs index. Spell lists are class-derived — the label is
    /// the owning class's label.
    pub fn entry_label_desc(
        &self,
        entry: IndexEntry<'_>,
    ) -> (ArcSignal<String>, ArcSignal<String>) {
        match entry {
            IndexEntry::Class(name) => self.class_defs.label_desc(name, name),
            IndexEntry::Species(name) => self.species_defs.label_desc(name, name),
            IndexEntry::Background(name) => self.background_defs.label_desc(name, name),
            IndexEntry::Spell(list) => {
                let class_name = self.with_class_defs(|defs| {
                    defs.keys()
                        .find(|name| name.eq_ignore_ascii_case(list))
                        .cloned()
                });
                match class_name {
                    Some(name) => self
                        .class_defs
                        .label_desc(name.to_string(), name.to_string()),
                    None => self.class_defs.label_desc(list, list),
                }
            }
        }
    }

    /// Short names of every class-derived spell list, two-hop: class level
    /// features → features index → `SpellsList::Ref { from }`. Tracked.
    pub fn spell_list_names(&self) -> Vec<String> {
        self.with_class_defs(|classes| {
            self.with_features_index(|features| {
                let mut names: Vec<String> = Vec::new();
                for class_def in classes.values() {
                    let scopes: Vec<Option<&str>> = std::iter::once(None)
                        .chain(class_def.subclasses.keys().map(|sub| Some(&**sub)))
                        .collect();
                    for scope in scopes {
                        for feat_name in class_def.feature_names(scope) {
                            let Some(list) = features
                                .get(feat_name)
                                .and_then(|feat| feat.spells.as_ref())
                                .and_then(|spells_def| spells_def.list.ref_name())
                            else {
                                continue;
                            };
                            if !names.iter().any(|existing| existing == list) {
                                names.push(list.to_string());
                            }
                        }
                    }
                }
                names.sort();
                names
            })
        })
    }

    pub fn canonical_class_name(&self, query: &str) -> Option<String> {
        self.class_defs
            .with_data(|defs| defs.and_then(|index| canonical_name(&index.0, query)))
    }

    pub fn canonical_species_name(&self, query: &str) -> Option<String> {
        self.species_defs
            .with_data(|defs| defs.and_then(|index| canonical_name(&index.0, query)))
    }

    pub fn canonical_background_name(&self, query: &str) -> Option<String> {
        self.background_defs
            .with_data(|defs| defs.and_then(|index| canonical_name(&index.0, query)))
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

    /// True when level-up is possible: either the character has no real
    /// classes yet (next pick adds the first one), or at least one
    /// existing class hasn't hit its progression-table max. Empty `class`
    /// entries are ignored as legacy noise.
    pub fn can_level_up(&self, character: &Character) -> bool {
        let mut real_classes = character
            .identity
            .classes
            .iter()
            .filter(|class_level| !class_level.class.is_empty())
            .peekable();
        if real_classes.peek().is_none() {
            return true;
        }
        real_classes.any(|class_level| {
            self.classes()
                .with(&class_level.class, |def| {
                    class_level.level < def.max_level()
                })
                .unwrap_or(true)
        })
    }

    /// Checks whether `character` can multiclass into `class_name`.
    /// All existing classes and the candidate class must meet their
    /// prerequisites.
    /// Every class currently in `identity.classes` satisfies its multiclass
    /// prerequisites against `character`. Empty class entries and classes
    /// missing from the defs index pass vacuously. Used by the picker for
    /// `Category(System(Class))` placeholders to gate level-up / multiclass
    /// candidates against the existing-classes side of the PHB rules.
    pub fn meets_class_prerequisites(&self, character: &CharacterCore) -> bool {
        self.class_defs.with_data(|defs| {
            character.identity.classes.iter().all(|cl| {
                cl.class.is_empty()
                    || defs
                        .and_then(|index| index.0.get(cl.class.as_ref()))
                        .is_none_or(|def| def.meets_prerequisites(character))
            })
        })
    }

    pub fn can_multiclass(&self, character: &CharacterCore, class_name: &str) -> bool {
        self.meets_class_prerequisites(character)
            && self.class_defs.with_data(|defs| {
                defs.and_then(|index| index.0.get(class_name))
                    .is_none_or(|def| def.meets_prerequisites(character))
            })
    }

    /// Replacement candidates for a placeholder, filtered against `character`
    /// (the speculative modal snapshot, NOT the live store).
    pub fn replacement_candidates(
        &self,
        character: &CharacterCore,
        replace_with: &ReplaceWith,
        parent_class_for_subclass: Option<&str>,
    ) -> Vec<Box<str>> {
        self.with_features_index(|features_index| {
            // System(Class) candidates: a new class needs the full multiclass
            // gate (every existing class meets its prereq + this class's
            // prereq); an existing class always passes — level-up doesn't
            // re-check multiclass requirements.
            let class_prereqs_ok = self.meets_class_prerequisites(character);
            features_index
                .values()
                .filter(|feat| {
                    if !replace_with.matches(feat) {
                        return false;
                    }
                    match feat.category {
                        FeatureCategory::System(IdentitySlot::Class) => {
                            let is_own = character
                                .identity
                                .classes
                                .iter()
                                .any(|class_level| class_level.class.as_ref() == &*feat.name);
                            is_own || (class_prereqs_ok && feat.meets_prerequisites(character))
                        }
                        // System(Subclass) candidates: limit to subclasses of
                        // the placeholder's parent class — `Subclass`
                        // placeholders are attached to a specific class via
                        // `source = Class(name, lvl)`, and a Cleric's picker
                        // shouldn't surface a Wizard subclass.
                        FeatureCategory::System(IdentitySlot::Subclass) => {
                            if !feat.meets_prerequisites(character) {
                                return false;
                            }
                            parent_class_for_subclass.is_some_and(|parent| {
                                self.classes()
                                    .with(parent, |class_def| {
                                        class_def.subclasses.contains_key(&*feat.name)
                                    })
                                    .unwrap_or(false)
                            })
                        }
                        _ => feat.meets_prerequisites(character),
                    }
                })
                .map(|feat| feat.name.clone())
                .collect()
        })
    }

    pub fn is_loading(&self) -> bool {
        self.class_defs.is_pending()
            || self.species_defs.is_pending()
            || self.background_defs.is_pending()
            || self.spell_names_cache.is_pending()
            || self.features_index.is_pending()
            || self.spells_index.is_pending()
            || self.effects_index.is_pending()
    }

    pub fn new(i18n: leptos_fluent::I18n, packages: Signal<VecSet<String>>) -> Self {
        let locale = Signal::derive(move || i18n.language.get().id.to_string());
        Self::build(locale, packages)
    }

    /// Construct the registry from locale/packages signals directly,
    /// bypassing the `leptos_fluent::I18n` dependency. Used by wasm tests
    /// that exercise locale/package-switch behavior without a full i18n
    /// context.
    #[cfg(test)]
    pub fn new_with_locale(locale: Signal<String>, packages: Signal<VecSet<String>>) -> Self {
        Self::build(locale, packages)
    }

    /// Test-only: build a registry from in-memory definitions, no fetches.
    /// All indexes resolve to provided values via `LocalizedIndex::for_test`.
    /// Caller must `await registry.await_ready()` before any untracked read.
    #[cfg(any(test, feature = "testing"))]
    pub fn for_test(
        features: FeaturesIndex,
        classes: BTreeMap<Box<str>, ClassDefinition>,
        species: BTreeMap<Box<str>, SpeciesDefinition>,
        backgrounds: BTreeMap<Box<str>, BackgroundDefinition>,
    ) -> Self {
        Self {
            packages: Signal::stored(crate::rules::packages::test_packages()),
            manifest: LocalResource::new(|| async { Ok(Vec::new()) }),
            class_defs: LocalizedIndex::for_test(DefsIndex(classes), None),
            species_defs: LocalizedIndex::for_test(DefsIndex(species), None),
            background_defs: LocalizedIndex::for_test(DefsIndex(backgrounds), None),
            spell_names_cache: FetchCache::new(),
            spells_index: LocalizedIndex::for_test(SpellsIndex::default(), None),
            effects_index: LocalizedIndex::for_test(EffectsIndex::default(), None),
            features_index: LocalizedIndex::for_test(features, None),
            synth_features: StoredValue::new(BTreeMap::new()),
        }
    }

    /// Test-only: replace the spells index of a `for_test` registry.
    #[cfg(any(test, feature = "testing"))]
    pub fn with_spells(mut self, spells: SpellsIndex) -> Self {
        self.spells_index = LocalizedIndex::for_test(spells, None);
        self
    }

    /// Test-only: seed a spell list into the names cache directly.
    #[cfg(any(test, feature = "testing"))]
    pub fn seed_spell_list_for_test(&self, path: &str, names: Vec<String>) {
        self.spell_names_cache.update(|map| {
            map.insert(path.into(), names);
        });
    }

    /// Test-only: whether a spell list is currently cached.
    #[cfg(any(test, feature = "testing"))]
    pub fn has_spell_list_for_test(&self, path: &str) -> bool {
        self.spell_names_cache.read_untracked().contains_key(path)
    }

    /// Test-only: replace the class defs index (data + locale) directly.
    #[cfg(any(test, feature = "testing"))]
    pub fn set_class_defs_for_test(
        &mut self,
        classes: BTreeMap<Box<str>, ClassDefinition>,
        locale: Option<LocaleMap>,
    ) {
        self.class_defs = LocalizedIndex::for_test(DefsIndex(classes), locale);
    }

    /// Test-only: drive the LocalResource futures so untracked reads
    /// return ready data.
    #[cfg(any(test, feature = "testing"))]
    pub async fn await_ready(&self) {
        self.class_defs.await_ready().await;
        self.species_defs.await_ready().await;
        self.background_defs.await_ready().await;
        self.spells_index.await_ready().await;
        self.effects_index.await_ready().await;
        self.features_index.await_ready().await;
    }

    fn build(locale: Signal<String>, packages: Signal<VecSet<String>>) -> Self {
        let class_defs = LocalizedIndex::<DefsIndex<ClassDefinition>, LocaleMap>::new(
            locale,
            packages,
            "classes.json",
        );
        let species_defs = LocalizedIndex::<DefsIndex<SpeciesDefinition>, LocaleMap>::new(
            locale,
            packages,
            "species.json",
        );
        let background_defs = LocalizedIndex::<DefsIndex<BackgroundDefinition>, LocaleMap>::new(
            locale,
            packages,
            "backgrounds.json",
        );
        let effects_index =
            LocalizedIndex::<EffectsIndex, EffectsLocaleMap>::new(locale, packages, "effects.json");
        let features_index =
            LocalizedIndex::<FeaturesIndex, LocaleMap>::new(locale, packages, "features.json");
        let spells_index =
            LocalizedIndex::<SpellsIndex, SpellsLocaleMap>::new(locale, packages, "spells.json");

        let manifest = LocalResource::new(move || async move {
            fetch_json::<Vec<PackageManifestEntry>>(&format!("{BASE_URL}/rules/index.json")).await
        });

        let spell_names_cache = FetchCache::new();

        // Package-set change invalidates the unioned spell lists; the
        // LocalizedIndexes refetch themselves.
        Effect::new(move |prev: Option<()>| {
            packages.track();
            if prev.is_some() {
                spell_names_cache.clear();
            }
        });

        let synth_features: StoredValue<BTreeMap<Box<str>, FeatureDefinition>> =
            StoredValue::new(BTreeMap::new());

        // Synthesize System features whenever a defs index resolves. All
        // definitions arrive eagerly (whole-file per package), so classes,
        // species, backgrounds AND subclasses are covered by one Effect.
        Effect::new(move || {
            species_defs.with_data(|maybe_defs| {
                let Some(defs) = maybe_defs else { return };
                synth_features.update_value(|map| {
                    for name in defs.0.keys() {
                        map.entry(name.clone()).or_insert_with(|| {
                            make_system_feature(name.clone(), IdentitySlot::Species)
                        });
                    }
                });
            });
            background_defs.with_data(|maybe_defs| {
                let Some(defs) = maybe_defs else { return };
                synth_features.update_value(|map| {
                    for name in defs.0.keys() {
                        map.entry(name.clone()).or_insert_with(|| {
                            make_system_feature(name.clone(), IdentitySlot::Background)
                        });
                    }
                });
            });
            class_defs.with_data(|maybe_defs| {
                let Some(defs) = maybe_defs else { return };
                synth_features.update_value(|map| {
                    for (name, class_def) in &defs.0 {
                        let prerequisites = class_def.prerequisites.clone();
                        map.entry(name.clone()).or_insert_with(|| {
                            let prereq = compose_class_prereq(name, prerequisites);
                            let mut feat = make_system_feature(name.clone(), IdentitySlot::Class);
                            feat.prerequisites = prereq;
                            feat
                        });
                        // Subclasses carry a `CLASS.`<parent>`.LEVEL >= 1`
                        // prereq so the picker only shows subclasses for
                        // classes the character actually has.
                        for subclass_name in class_def.subclasses.keys() {
                            map.entry(subclass_name.clone()).or_insert_with(|| {
                                let mut feat = make_system_feature(
                                    subclass_name.clone(),
                                    IdentitySlot::Subclass,
                                );
                                feat.prerequisites = Some(parent_class_prereq(name));
                                feat
                            });
                        }
                    }
                });
            });
        });

        Self {
            packages,
            manifest,
            class_defs,
            species_defs,
            background_defs,
            effects_index,
            features_index,
            spells_index,
            spell_names_cache,
            synth_features,
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

    /// Borrow the loaded class/species/background definition indexes as a
    /// single bundle for the apply pipeline. Mirror of
    /// [`Self::with_features_index_untracked`] for definition indexes.
    pub fn with_definitions<R>(&self, f: impl FnOnce(DefinitionCaches<'_>) -> R) -> R {
        let class_guard = self.class_defs.data.read_untracked();
        let species_guard = self.species_defs.data.read_untracked();
        let bg_guard = self.background_defs.data.read_untracked();
        let caches = DefinitionCaches {
            classes: defs_or_empty(&class_guard, &EMPTY_CLASS_DEFS),
            species: defs_or_empty(&species_guard, &EMPTY_SPECIES_DEFS),
            backgrounds: defs_or_empty(&bg_guard, &EMPTY_BACKGROUND_DEFS),
        };
        f(caches)
    }

    // ---- Internal helpers ----

    /// One URL per active package for a package-relative data path.
    fn package_data_urls(&self, path: &str) -> Vec<String> {
        self.packages
            .read_untracked()
            .iter()
            .map(|pkg| format!("{BASE_URL}/rules/{pkg}/data/{path}"))
            .collect()
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

    /// Resolve `(label, description)` for a feature, routing synthesized
    /// `System(_)` entries to their identity-source locale (species /
    /// background / class defs overlay). Hand-written features fall through
    /// to the regular `features().label_desc(name, name)` path.
    pub fn feature_label_desc(&self, name: &str) -> (ArcSignal<String>, ArcSignal<String>) {
        let synth_slot = self.synth_features.with_value(|map| {
            map.get(name).and_then(|feat| match feat.category {
                FeatureCategory::System(slot) => Some(slot),
                _ => None,
            })
        });
        match synth_slot {
            Some(IdentitySlot::Species) => self.species_defs.label_desc(name, name),
            Some(IdentitySlot::Background) => self.background_defs.label_desc(name, name),
            Some(IdentitySlot::Class) => self.class_defs.label_desc(name, name),
            Some(IdentitySlot::Subclass) => self
                .find_subclass_parent(name)
                .map(|parent_class| self.subclass_label_desc(parent_class, name.to_string()))
                .unwrap_or_else(|| self.features().label_desc(name, name)),
            None => self.features().label_desc(name, name),
        }
    }

    /// Locate the parent class of a subclass by scanning loaded class
    /// definitions. Returns `None` when the defs index hasn't resolved yet.
    fn find_subclass_parent(&self, subclass_name: &str) -> Option<String> {
        self.class_defs.with_data_untracked(|defs| {
            defs?.0.iter().find_map(|(class_name, class_def)| {
                class_def
                    .subclasses
                    .contains_key(subclass_name)
                    .then(|| class_name.to_string())
            })
        })
    }

    /// Reactive `(label, description)` for a named subclass under the
    /// given parent class. Routes through `classes().lookup(...)` which
    /// builds a `LocalizedText<ClassDefinition, LocaleMap>` exposing
    /// `subclass(name)` for the per-subclass locale entry. Falls back to
    /// the raw subclass name when no locale entry is available.
    fn subclass_label_desc(
        &self,
        parent_class: String,
        subclass_name: String,
    ) -> (ArcSignal<String>, ArcSignal<String>) {
        let registry = *self;
        let label = ArcSignal::derive({
            let parent = parent_class.clone();
            let subclass = subclass_name.clone();
            move || {
                registry
                    .classes()
                    .lookup(&parent, |loc| {
                        loc.subclass(&subclass)
                            .map(|localized| localized.label().to_string())
                    })
                    .flatten()
                    .unwrap_or_else(|| subclass.clone())
            }
        });
        let description = ArcSignal::derive(move || {
            registry
                .classes()
                .lookup(&parent_class, |loc| {
                    loc.subclass(&subclass_name)
                        .map(|localized| localized.description().to_string())
                })
                .flatten()
                .unwrap_or_default()
        });
        (label, description)
    }

    /// Untracked variant of `feature_label_desc` returning concrete
    /// `(label, description)` strings — used at component mount time when
    /// reactive subscription would be wasted.
    pub fn feature_label_desc_untracked(&self, name: &str) -> (String, String) {
        let synth_slot = self.synth_features.with_value(|map| {
            map.get(name).and_then(|feat| match feat.category {
                FeatureCategory::System(slot) => Some(slot),
                _ => None,
            })
        });
        let defs_locale = match synth_slot {
            Some(IdentitySlot::Species) => Some(self.species_defs.locale),
            Some(IdentitySlot::Background) => Some(self.background_defs.locale),
            Some(IdentitySlot::Class) => Some(self.class_defs.locale),
            _ => None,
        };
        if let Some(locale_resource) = defs_locale {
            let locale_guard = locale_resource.read_untracked();
            let text = locale_guard
                .as_ref()
                .and_then(|resource| resource.as_ref())
                .and_then(|map| map.get(name));
            let label = text
                .and_then(|entry| entry.label.clone())
                .unwrap_or_else(|| name.to_string());
            let description = text
                .and_then(|entry| entry.description.clone())
                .unwrap_or_default();
            return (label, description);
        }
        if matches!(synth_slot, Some(IdentitySlot::Subclass))
            && let Some(parent_class) = self.find_subclass_parent(name)
        {
            return self
                .classes()
                .lookup_untracked(&parent_class, |loc| {
                    loc.subclass(name).map(|localized| {
                        (
                            localized.label().to_string(),
                            localized.description().to_string(),
                        )
                    })
                })
                .flatten()
                .unwrap_or_else(|| (name.to_string(), String::new()));
        }
        self.features()
            .lookup_untracked(name, |loc| {
                (loc.label().to_string(), loc.description().to_string())
            })
            .unwrap_or_else(|| (name.to_string(), String::new()))
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

    pub fn with_features_index<R>(&self, f: impl FnOnce(FeaturesView<'_>) -> R) -> R {
        let guard = self.features_index.data.read();
        let index: Option<&FeaturesIndex> = guard.as_ref().and_then(|r| r.as_ref().ok());
        let natural = index.map_or(&EMPTY_FEATURES_INDEX, |idx| &idx.0);
        self.synth_features
            .with_value(|synth| f(FeaturesView { natural, synth }))
    }

    pub fn with_features_index_untracked<R>(&self, f: impl FnOnce(FeaturesView<'_>) -> R) -> R {
        let guard = self.features_index.data.read_untracked();
        let index: Option<&FeaturesIndex> = guard.as_ref().and_then(|r| r.as_ref().ok());
        let natural = index.map_or(&EMPTY_FEATURES_INDEX, |idx| &idx.0);
        self.synth_features
            .with_value(|synth| f(FeaturesView { natural, synth }))
    }

    /// Test-only handle to the synthesized features map. Allows wasm tests to
    /// seed entries directly and verify they survive locale-switch refetches.
    #[cfg(test)]
    pub fn synth_features_handle(&self) -> StoredValue<BTreeMap<Box<str>, FeatureDefinition>> {
        self.synth_features
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
    /// names cache, unioned across the active packages. The list is
    /// locale-less — just `["Acid Splash", ...]`.
    pub fn fetch_spell_list_untracked(&self, path: &str) {
        self.spell_names_cache
            .fetch_merged(path, self.package_data_urls(path), "spell list");
    }

    /// Same as `fetch_spell_list_untracked` — `Ref { from }` values are
    /// direct package-relative paths; no index resolution step remains.
    pub fn fetch_spell_list(&self, path: &str) {
        self.fetch_spell_list_untracked(path);
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
        let guard = self.class_defs.data.read_untracked();
        let classes = defs_or_empty(&guard, &EMPTY_CLASS_DEFS);
        resolve::feature_class_level(identity, feature_name, classes)
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
                && let Some(action_def) = feat.actions.get(field_name)
            {
                let level = self.feature_class_level_for(classes, feature_name);
                return action_def.resolve_choice_options(character_fields, level);
            }
            Vec::new()
        })
    }

    pub fn get_choice_cost_label(&self, feature_name: &str, field_name: &str) -> Option<String> {
        self.with_features_index_untracked(|features_index| {
            let feat = features_index.get(feature_name)?;
            let action_def = feat.actions.get(field_name)?;
            let cost_name = action_def.cost.as_ref()?;
            Some(crate::model::short_name(cost_name))
        })
    }

    fn feature_class_level_for(&self, classes: &[ClassLevel], feature_name: &str) -> u32 {
        let guard = self.class_defs.data.read_untracked();
        let class_defs = defs_or_empty(&guard, &EMPTY_CLASS_DEFS);
        resolve::feature_class_level_from_classes(classes, feature_name, class_defs).unwrap_or(0)
    }

    // ---- Fill / Clear ----

    /// Trigger fetches for the character's lazy resources. Definitions are
    /// eager now — only the per-class spell name lists remain on demand.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.ensure_definitions_fetched", skip_all)
    )]
    pub fn ensure_definitions_fetched(&self, character: &CharacterCore) {
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
        self.class_defs.track();

        self.sync_labels(
            character,
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
            // Fill: copy cost from the feature's spell entry; overwrite
            // level from catalog. Per-spell free_uses pools are managed
            // by the FREE_USES assign Context handler now.
            |spell, extras| {
                spell.cost = extras.cost;
                if let Some(level) = extras.level {
                    spell.level = level;
                }
            },
        );
    }

    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "registry.clear_from_registry", skip_all)
    )]
    pub fn clear_from_registry(&self, character: &mut Character) {
        self.sync_labels(
            character,
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

/// Build `CLASS.\`<parent>\`.LEVEL >= 1` — prerequisite for synthesized
/// `System(Subclass)` features.
fn parent_class_prereq(parent_class: &str) -> Expr {
    [
        Op::PushVar(Attribute::ClassLevel(AttrKey::named(parent_class))),
        Op::PushConst(1),
        Op::Cmp(Cmp::Ge),
    ]
    .into_iter()
    .collect()
}

/// Synthesize a `System(Class)` feature's prerequisite from the class's
/// catalog multiclass requirement. Adds two bypass clauses:
///
/// - `CLASS.COUNT == 0` — first-class exemption (no class yet → ignore
///   multiclass ability requirements).
/// - `CLASS.\`<class_name>\`.LEVEL >= 1` — own-class exemption (level-up on an
///   existing class doesn't re-check multiclass requirements; PHB rules only
///   gate the *adding* of a new class).
fn compose_class_prereq(class_name: &str, existing: Option<Expr>) -> Option<Expr> {
    let existing = existing?;
    debug_assert_eq!(
        existing.blocks().count(),
        1,
        "class prereq must be flat (no if/each/with sub-blocks)"
    );
    Some(
        existing
            .block(BLOCK_MAIN)
            .iter()
            .cloned()
            .chain([
                Op::PushVar(Attribute::ClassCount),
                Op::PushConst(0),
                Op::Cmp(Cmp::Eq),
                Op::BinOp(BinOp::Or),
                Op::PushVar(Attribute::ClassLevel(AttrKey::named(class_name))),
                Op::PushConst(1),
                Op::Cmp(Cmp::Ge),
                Op::BinOp(BinOp::Or),
            ])
            .collect(),
    )
}

/// Build the System feature for a single species/background/subclass/class
/// entry. `slot` selects which identity attribute the OnFeatureAdd assign
/// writes. Species/Background/Subclass write a boolean toggle (`= 1`);
/// Class is stackable and bumps `CLASS.\`name\`.LEVEL += 1` per apply.
///
/// Consumes `name`: leaks the box into the intern pool to get a `&'static str`
/// for the assign Op, then rebuilds a fresh `Box<str>` from the leaked view
/// for `FeatureDefinition.name`. Cheaper than the `intern(&name)` path which
/// reallocates the string twice via `to_string().into_boxed_str()`.
pub fn make_system_feature(name: Box<str>, slot: IdentitySlot) -> FeatureDefinition {
    let interned = intern_box(name);
    let (assign_expr, stackable) = match slot {
        IdentitySlot::Species => (
            [
                Op::PushConst(1),
                Op::AssignVar(Attribute::Species(interned)),
            ]
            .into_iter()
            .collect(),
            false,
        ),
        IdentitySlot::Background => (
            [
                Op::PushConst(1),
                Op::AssignVar(Attribute::Background(interned)),
            ]
            .into_iter()
            .collect(),
            false,
        ),
        IdentitySlot::Subclass => (
            [
                Op::PushConst(1),
                Op::AssignVar(Attribute::Subclass(interned)),
            ]
            .into_iter()
            .collect(),
            false,
        ),
        IdentitySlot::Class => {
            let var = Attribute::ClassLevel(AttrKey::Named(interned));
            (
                [
                    Op::PushVar(var),
                    Op::PushConst(1),
                    Op::BinOp(BinOp::Add),
                    Op::AssignVar(var),
                ]
                .into_iter()
                .collect(),
                true,
            )
        }
    };
    FeatureDefinition {
        name: Box::from(interned),
        stackable,
        category: FeatureCategory::System(slot),
        replace_with: ReplaceWith::None,
        spells: None,
        actions: BTreeMap::new(),
        assign: Some(vec![Assignment {
            expr: assign_expr,
            when: WhenCondition::OnFeatureAdd,
        }]),
        prerequisites: None,
    }
}

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
    /// Test-only constructor: bypass `fetch_json`, resolve to provided
    /// `data` and `locale` immediately. Caller must `await await_ready()`
    /// before any untracked read.
    #[cfg(any(test, feature = "testing"))]
    pub fn for_test(data: T, locale: Option<L>) -> Self
    where
        T: Clone,
        L: Clone,
    {
        let in_flight = RwSignal::new(0u32);
        let data_value = data.clone();
        let data_resource = LocalResource::new(move || {
            let value = data_value.clone();
            async move { Ok::<T, String>(value) }
        });
        let locale_value = locale.clone();
        let locale_resource = LocalResource::new(move || {
            let value = locale_value.clone();
            async move { value }
        });
        let loading = Signal::derive(move || in_flight.get() > 0);
        let this = Self {
            data: data_resource,
            locale: locale_resource,
            in_flight,
            loading,
        };
        this.observe_initial();
        this
    }

    /// Drive both inner futures to completion. Test-only — production
    /// code reacts to `data.read()` becoming `Some` instead.
    #[cfg(any(test, feature = "testing"))]
    pub async fn await_ready(&self) {
        let _ = self.data.into_future().await;
        let _ = self.locale.into_future().await;
    }

    pub fn new(
        locale: Signal<String>,
        packages: Signal<VecSet<String>>,
        file_name: &'static str,
    ) -> Self
    where
        T: PackageMerge + Default,
        L: PackageMerge + Default,
    {
        let in_flight = RwSignal::new(0u32);

        // Both resources read locale/packages UNTRACKED — refetch is driven
        // explicitly by the Effects below so we have a single point that
        // toggles the in-flight counter.
        let data = LocalResource::new(move || {
            let urls: Vec<String> = packages
                .read_untracked()
                .iter()
                .map(|pkg| format!("{BASE_URL}/rules/{pkg}/data/{file_name}"))
                .collect();
            async move { fetch_merged_json::<T>(&urls).await }
        });
        let locale_resource = LocalResource::new(move || {
            let lang = locale.get_untracked();
            let urls: Vec<String> = packages
                .read_untracked()
                .iter()
                .map(|pkg| format!("{BASE_URL}/rules/{pkg}/{lang}/{file_name}"))
                .collect();
            async move { fetch_merged_json::<L>(&urls).await.ok() }
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

        // Locale switch re-fetches only the overlay.
        Effect::new(move |prev: Option<()>| {
            locale.track();
            if prev.is_some() {
                this.refetch_locale();
            }
        });
        // Package-set switch re-fetches data AND overlay.
        Effect::new(move |prev: Option<()>| {
            packages.track();
            if prev.is_some() {
                this.refetch_all();
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
        tracing::instrument(name = "localized_index.refetch_locale", skip_all)
    )]
    pub fn refetch_locale(&self) {
        let in_flight = self.in_flight;
        let locale_res = self.locale;
        in_flight.update(|count| *count += 1);
        locale_res.refetch();
        leptos::task::spawn_local(async move {
            let _ = locale_res.into_future().await;
            in_flight.update(|count| *count = count.saturating_sub(1));
        });
    }

    /// Re-fetch data and locale (package-set change). Bumps `loading`.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "localized_index.refetch_all", skip_all)
    )]
    pub fn refetch_all(&self) {
        let in_flight = self.in_flight;
        let data_res = self.data;
        let locale_res = self.locale;
        in_flight.update(|count| *count += 2);
        data_res.refetch();
        locale_res.refetch();
        leptos::task::spawn_local(async move {
            let _ = data_res.into_future().await;
            in_flight.update(|count| *count = count.saturating_sub(1));
        });
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

static EMPTY_CLASS_DEFS: BTreeMap<Box<str>, ClassDefinition> = BTreeMap::new();
static EMPTY_SPECIES_DEFS: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
static EMPTY_BACKGROUND_DEFS: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();

/// Unwrap a defs-index resource state to its map, falling back to empty.
fn defs_or_empty<'a, T>(
    state: &'a Option<Result<DefsIndex<T>, String>>,
    empty: &'a BTreeMap<Box<str>, T>,
) -> &'a BTreeMap<Box<str>, T> {
    state
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map_or(empty, |index| &index.0)
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

#[cfg(test)]
mod tests {
    use leptos::prelude::*;
    use wasm_bindgen_test::*;

    use super::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn synth_features_survive_locale_switch() {
        // LocalizedIndex::new spawns LocalResource futures via spawn_local;
        // wasm-bindgen-test runs without leptos' default global executor.
        let _ = any_spawner::Executor::init_wasm_bindgen();

        let owner = Owner::new();
        let registry = owner.with(|| {
            let locale = RwSignal::new("en".to_string());
            let packages = RwSignal::new(crate::rules::packages::test_packages());
            let registry = RulesRegistry::new_with_locale(locale.into(), packages.into());

            // Seed synth_features directly — bypasses the defs Effect
            // (which would require live HTTP fetches). The invariant under
            // test is whether the StoredValue survives a locale flip, not how
            // it gets populated.
            let synth = registry.synth_features_handle();
            synth.update_value(|map| {
                map.insert(
                    Box::from("Elf"),
                    make_system_feature(Box::from("Elf"), IdentitySlot::Species),
                );
                map.insert(
                    Box::from("Sage"),
                    make_system_feature(Box::from("Sage"), IdentitySlot::Background),
                );
            });

            // Read via the production path (with_features_index_untracked) —
            // catches regressions where someone wires synth_features.set_value
            // into the locale Effect.
            let initial_visible = registry.with_features_index_untracked(|view| {
                view.contains_key("Elf") && view.contains_key("Sage")
            });
            assert!(
                initial_visible,
                "seeded entries must be visible via with_features_index_untracked"
            );

            locale.set("ru".to_string());

            let after_visible = registry.with_features_index_untracked(|view| {
                view.contains_key("Elf") && view.contains_key("Sage")
            });
            assert!(
                after_visible,
                "synth_features must survive locale switch (visible via with_features_index_untracked)"
            );
            registry
        });
        // Settle in-flight fetches before the Owner drops — leaked futures
        // polled after disposal panic inside a later test.
        registry.await_ready().await;
    }

    #[wasm_bindgen_test]
    async fn merged_defs_lookup_and_locale_keys() {
        let _ = any_spawner::Executor::init_wasm_bindgen();
        let owner = Owner::new();
        let registry = owner.with(|| {
            let locale = RwSignal::new("ru".to_string());
            let packages = RwSignal::new(crate::rules::packages::test_packages());
            let mut registry = RulesRegistry::new_with_locale(locale.into(), packages.into());
            // Inject merged defs + locale directly (no HTTP in tests).
            let mut classes = BTreeMap::new();
            classes.insert(
                Box::from("Wizard"),
                serde_json::from_str::<ClassDefinition>(
                    r#"{"name": "Wizard", "prerequisites": "INT >= 13"}"#,
                )
                .unwrap(),
            );
            // LocaleKey's inner field is private — build the map via serde.
            let locale_map: LocaleMap =
                serde_json::from_str(r#"{"Wizard": {"label": "Волшебник"}}"#).unwrap();
            registry.set_class_defs_for_test(classes, Some(locale_map));
            registry
        });
        registry.await_ready().await;

        let label = registry
            .classes()
            .lookup_untracked("Wizard", |localized| localized.label().to_string());
        assert_eq!(label.as_deref(), Some("Волшебник"));
    }

    #[wasm_bindgen_test]
    async fn package_switch_triggers_index_refetch() {
        let _ = any_spawner::Executor::init_wasm_bindgen();
        let owner = Owner::new();
        let (registry, packages) = owner.with(|| {
            let locale = RwSignal::new("en".to_string());
            let packages = RwSignal::new(crate::rules::packages::test_packages());
            (
                RulesRegistry::new_with_locale(locale.into(), packages.into()),
                packages,
            )
        });
        // Settle initial fetches first — `loading` is true from construction,
        // so asserting it right after a flip would be vacuous.
        registry.features().await_ready().await;
        assert!(!registry.features().loading.get_untracked());

        packages.update(|set| {
            set.remove("lorwyn");
        });
        // Effects run on the executor, never synchronously after a signal
        // write — yield before asserting.
        leptos::task::tick().await;
        assert!(
            registry.features().loading.get_untracked(),
            "package-set change must refetch the features index"
        );
        // Settle the refetches before the Owner drops (see synth test note).
        registry.await_ready().await;
    }

    #[wasm_bindgen_test]
    async fn package_switch_clears_spell_lists() {
        let _ = any_spawner::Executor::init_wasm_bindgen();
        let owner = Owner::new();
        let (registry, packages) = owner.with(|| {
            let locale = RwSignal::new("en".to_string());
            let packages = RwSignal::new(crate::rules::packages::test_packages());
            (
                RulesRegistry::new_with_locale(locale.into(), packages.into()),
                packages,
            )
        });
        // Let the invalidation Effect complete its first (prev=None) run
        // before flipping — otherwise the flip is its first run and skipped.
        registry.await_ready().await;
        registry.seed_spell_list_for_test("spells/wizard.json", vec!["Fireball".into()]);
        packages.update(|set| {
            set.remove("lorwyn");
        });
        // The invalidation runs in an Effect on the executor — yield first.
        leptos::task::tick().await;
        assert!(
            !registry.has_spell_list_for_test("spells/wizard.json"),
            "package-set change must clear cached spell lists"
        );
        // Settle the refetches before the Owner drops (see synth test note).
        registry.await_ready().await;
    }
}
