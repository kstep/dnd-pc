use std::collections::BTreeMap;

use leptos::prelude::*;

use super::pending::PendingFeature;
use crate::{
    model::{Character, ClassLevel, FeatureSource},
    rules::{
        DefinitionStore, RulesRegistry, background::BackgroundDefinition, class::ClassDefinition,
        feature::FeatureDefinition, species::SpeciesDefinition,
    },
};

/// Names and canonical sources granted by a class at one level, including the
/// selected subclass if any. No filtering — callers apply their own.
pub(crate) fn class_level_sources<'a>(
    class_level: &'a ClassLevel,
    level: u32,
    class_def: &'a ClassDefinition,
) -> impl Iterator<Item = (&'a str, FeatureSource)> + 'a {
    let class_source = FeatureSource::Class(class_def.name.as_str().into(), level);
    let subclass_source = class_level.subclass.as_deref().map(|subclass| {
        FeatureSource::Subclass(class_def.name.as_str().into(), subclass.into(), level)
    });

    let class_iter = class_def
        .levels
        .get(&level)
        .into_iter()
        .flat_map(|rules| rules.features.iter())
        .map(move |name| (name.as_str(), class_source.clone()));

    let subclass_iter = class_level
        .subclass
        .as_deref()
        .and_then(|subclass| class_def.subclasses.get(subclass))
        .and_then(|subclass| subclass.levels.get(&level))
        .into_iter()
        .flat_map(|rules| rules.features.iter())
        .filter_map(move |name| subclass_source.clone().map(|src| (name.as_str(), src)));

    class_iter.chain(subclass_iter)
}

/// Collect new features for a class level-up from class + subclass level rules.
/// Filters out already-applied features via dedup check.
pub fn collect_class_features<'a>(
    character: &'a Character,
    class_idx: usize,
    level: u32,
    class_def: &'a ClassDefinition,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
) -> impl Iterator<Item = PendingFeature> + 'a {
    let class_level = &character.identity.classes[class_idx];
    class_level_sources(class_level, level, class_def)
        .filter(move |(name, source)| {
            features_index.get(*name).is_none_or(|feat| {
                !character
                    .features
                    .contains(&feat.name, feat.stackable, source)
            })
        })
        .map(move |(name, source)| PendingFeature {
            name: name.to_string(),
            source,
            level,
        })
}

/// Collect features from a species definition.
pub fn collect_species_features<'a>(
    character: &'a Character,
    species_def: &'a SpeciesDefinition,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
) -> impl Iterator<Item = PendingFeature> + 'a {
    let total_level = character.level().max(1);
    let source = FeatureSource::Species(character.identity.species.clone().into());
    let filter_source = source.clone();
    species_def
        .features
        .iter()
        .filter(move |feat_name| {
            features_index.get(feat_name.as_str()).is_none_or(|feat| {
                !character
                    .features
                    .contains(&feat.name, feat.stackable, &filter_source)
            })
        })
        .map(move |feat_name| PendingFeature {
            name: feat_name.clone(),
            source: source.clone(),
            level: total_level,
        })
}

/// Collect features from a background definition.
pub fn collect_background_features<'a>(
    character: &'a Character,
    bg_def: &'a BackgroundDefinition,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
) -> impl Iterator<Item = PendingFeature> + 'a {
    let total_level = character.level().max(1);
    let source = FeatureSource::Background(character.identity.background.clone().into());
    let filter_source = source.clone();
    bg_def
        .features
        .iter()
        .filter(move |feat_name| {
            features_index.get(feat_name.as_str()).is_none_or(|feat| {
                !character
                    .features
                    .contains(&feat.name, feat.stackable, &filter_source)
            })
        })
        .map(move |feat_name| PendingFeature {
            name: feat_name.clone(),
            source: source.clone(),
            level: total_level,
        })
}

/// Collect all unapplied features: species (if not applied), background
/// (if not applied), and class features for unapplied levels.
pub fn collect_pending_features(
    character: &Character,
    registry: &RulesRegistry,
    features_index: &BTreeMap<Box<str>, FeatureDefinition>,
) -> Vec<PendingFeature> {
    let species_cache = registry.species().cache().read_untracked();
    let bg_cache = registry.backgrounds().cache().read_untracked();
    let class_cache = registry.classes().cache().read_untracked();

    let species_iter = species_cache
        .get(character.identity.species.as_str())
        .filter(|_| !character.identity.species.is_empty() && !character.applied.species)
        .into_iter()
        .flat_map(|species_def| collect_species_features(character, species_def, features_index));

    let bg_iter = bg_cache
        .get(character.identity.background.as_str())
        .filter(|_| !character.identity.background.is_empty() && !character.applied.background)
        .into_iter()
        .flat_map(|bg_def| collect_background_features(character, bg_def, features_index));

    let class_iter =
        character
            .identity
            .classes
            .iter()
            .enumerate()
            .flat_map(|(idx, class_level)| {
                let unapplied: Vec<u32> = (1..=class_level.level)
                    .filter(|lvl| !character.applied.contains_level(&class_level.class, *lvl))
                    .collect();
                let class_def = class_cache.get(class_level.class.as_str());
                unapplied.into_iter().flat_map(move |lvl| {
                    class_def.into_iter().flat_map(move |def| {
                        collect_class_features(character, idx, lvl, def, features_index)
                    })
                })
            });

    species_iter.chain(bg_iter).chain(class_iter).collect()
}
