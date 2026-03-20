use std::collections::BTreeMap;

use super::{
    background::BackgroundDefinition, class::ClassDefinition, feature::FeatureDefinition,
    race::RaceDefinition,
};
use crate::model::{CharacterIdentity, ClassLevel, FeatureSource};

/// Find a feature definition by name in the global features catalog.
/// Falls back to background/race inline features if not in catalog.
pub(super) fn find_feature<'a>(
    _identity: &CharacterIdentity,
    name: &str,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
    _bg_cache: &'a BTreeMap<Box<str>, BackgroundDefinition>,
    _race_cache: &'a BTreeMap<Box<str>, RaceDefinition>,
) -> Option<&'a FeatureDefinition> {
    features_index.get(name)
}

/// Find a feature and determine its source (Class/Background/Race).
pub(super) fn find_feature_with_source<'a>(
    identity: &CharacterIdentity,
    name: &str,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
    class_cache: &BTreeMap<Box<str>, ClassDefinition>,
    bg_cache: &'a BTreeMap<Box<str>, BackgroundDefinition>,
    race_cache: &'a BTreeMap<Box<str>, RaceDefinition>,
) -> Option<(&'a FeatureDefinition, Option<FeatureSource>)> {
    let feat = features_index.get(name)?;

    // Determine source by checking which class/bg/race references this feature
    for cl in &identity.classes {
        if let Some(def) = class_cache.get(cl.class.as_str())
            && def.feature_names(cl.subclass.as_deref()).any(|n| n == name)
        {
            return Some((feat, Some(FeatureSource::Class(cl.class.clone()))));
        }
    }

    if let Some(bg) = bg_cache.get(identity.background.as_str())
        && bg.features.iter().any(|n| n == name)
    {
        return Some((
            feat,
            Some(FeatureSource::Background(identity.background.clone())),
        ));
    }

    if let Some(race) = race_cache.get(identity.race.as_str())
        && race.features.iter().any(|n| n == name)
    {
        return Some((feat, Some(FeatureSource::Race(identity.race.clone()))));
    }

    // Feature exists in catalog but not referenced by any class/race/background —
    // manually-added feats (e.g. "Lucky", "Tough").
    Some((feat, None))
}

/// Find a feature and the class level of the owning class (0 for non-class).
pub(super) fn find_feature_with_class_level<'a>(
    identity: &CharacterIdentity,
    name: &str,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
    class_cache: &BTreeMap<Box<str>, ClassDefinition>,
    bg_cache: &'a BTreeMap<Box<str>, BackgroundDefinition>,
    race_cache: &'a BTreeMap<Box<str>, RaceDefinition>,
) -> Option<(&'a FeatureDefinition, u32)> {
    let feat = find_feature(identity, name, features_index, bg_cache, race_cache)?;

    for cl in &identity.classes {
        if let Some(def) = class_cache.get(cl.class.as_str())
            && def.feature_names(cl.subclass.as_deref()).any(|n| n == name)
        {
            return Some((feat, cl.level));
        }
    }

    Some((feat, 0))
}

/// Return the class level for the class that owns the given feature.
pub(super) fn feature_class_level(
    identity: &CharacterIdentity,
    feature_name: &str,
    class_cache: &BTreeMap<Box<str>, ClassDefinition>,
) -> Option<u32> {
    feature_class_level_from_classes(&identity.classes, feature_name, class_cache)
}

/// Shared helper: scan class levels for the class owning a feature.
pub(super) fn feature_class_level_from_classes(
    classes: &[ClassLevel],
    feature_name: &str,
    class_cache: &BTreeMap<Box<str>, ClassDefinition>,
) -> Option<u32> {
    classes.iter().find_map(|cl| {
        let def = class_cache.get(cl.class.as_str())?;
        def.feature_names(cl.subclass.as_deref())
            .any(|n| n == feature_name)
            .then_some(cl.level)
    })
}
