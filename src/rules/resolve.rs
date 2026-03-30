use std::collections::BTreeMap;

use super::{class::ClassDefinition, feature::FeatureDefinition};
use crate::model::{CharacterIdentity, ClassLevel};

/// Find a feature definition by name in the global features index.
pub(super) fn find_feature<'a>(
    name: &str,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
) -> Option<&'a FeatureDefinition> {
    features_index.get(name)
}

/// Find a feature and the class level of the owning class (0 for non-class).
pub(super) fn find_feature_with_class_level<'a>(
    identity: &CharacterIdentity,
    name: &str,
    features_index: &'a BTreeMap<Box<str>, FeatureDefinition>,
    class_cache: &BTreeMap<Box<str>, ClassDefinition>,
) -> Option<(&'a FeatureDefinition, u32)> {
    let feat = find_feature(name, features_index)?;

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
