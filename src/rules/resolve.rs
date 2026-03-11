use std::collections::BTreeMap;

use super::{
    background::BackgroundDefinition, class::ClassDefinition, feature::FeatureDefinition,
    race::RaceDefinition,
};
use crate::model::{CharacterIdentity, FeatureSource};

/// Find a feature definition by name, searching class → background → race
/// caches.
pub(super) fn find_feature<'a>(
    identity: &CharacterIdentity,
    name: &str,
    class_cache: &'a BTreeMap<Box<str>, ClassDefinition>,
    bg_cache: &'a BTreeMap<Box<str>, BackgroundDefinition>,
    race_cache: &'a BTreeMap<Box<str>, RaceDefinition>,
) -> Option<&'a FeatureDefinition> {
    for cl in &identity.classes {
        if let Some(def) = class_cache.get(cl.class.as_str())
            && let Some(feat) = def.find_feature(name, cl.subclass.as_deref())
        {
            return Some(feat);
        }
    }

    if let Some(feat) = bg_cache
        .get(identity.background.as_str())
        .and_then(|def| def.features.get(name))
    {
        return Some(feat);
    }

    race_cache
        .get(identity.race.as_str())
        .and_then(|def| def.features.get(name))
}

/// Find a feature and return both the definition and its source.
pub(super) fn find_feature_with_source<'a>(
    identity: &CharacterIdentity,
    name: &str,
    class_cache: &'a BTreeMap<Box<str>, ClassDefinition>,
    bg_cache: &'a BTreeMap<Box<str>, BackgroundDefinition>,
    race_cache: &'a BTreeMap<Box<str>, RaceDefinition>,
) -> Option<(&'a FeatureDefinition, FeatureSource)> {
    for cl in &identity.classes {
        if let Some(def) = class_cache.get(cl.class.as_str())
            && let Some(feat) = def.find_feature(name, cl.subclass.as_deref())
        {
            return Some((feat, FeatureSource::Class(cl.class.clone())));
        }
    }

    if let Some(feat) = bg_cache
        .get(identity.background.as_str())
        .and_then(|def| def.features.get(name))
    {
        return Some((feat, FeatureSource::Background(identity.background.clone())));
    }

    if let Some(feat) = race_cache
        .get(identity.race.as_str())
        .and_then(|def| def.features.get(name))
    {
        return Some((feat, FeatureSource::Race(identity.race.clone())));
    }

    None
}

/// Return the class level for the class that owns the given feature.
/// Returns `None` if the feature is not a class feature.
pub(super) fn feature_class_level(
    identity: &CharacterIdentity,
    feature_name: &str,
    class_cache: &BTreeMap<Box<str>, ClassDefinition>,
) -> Option<u32> {
    identity.classes.iter().find_map(|cl| {
        let def = class_cache.get(cl.class.as_str())?;
        def.find_feature(feature_name, cl.subclass.as_deref())
            .map(|_| cl.level)
    })
}
