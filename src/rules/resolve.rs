use std::collections::BTreeMap;

use crate::{
    model::{CharacterIdentity, ClassLevel},
    rules::class::ClassDefinition,
};

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
        let def = class_cache.get(cl.class.as_ref())?;
        def.feature_names(cl.subclass.as_deref())
            .any(|n| n == feature_name)
            .then_some(cl.level)
    })
}
