use std::collections::BTreeSet;

use leptos::prelude::*;

use crate::{
    model::{CharacterCore, ClassLevel, FeatureSource},
    rules::{
        DefinitionStore, FeaturesView, RulesRegistry,
        apply::{
            pending::{FeatureKey, PendingFeature},
            primitives::detect_replacement,
        },
        background::BackgroundDefinition,
        class::ClassDefinition,
        species::SpeciesDefinition,
    },
};

/// Names and canonical sources granted by a class at one level, including the
/// selected subclass if any. No filtering — callers apply their own.
pub fn class_level_sources<'a>(
    class_level: &'a ClassLevel,
    level: u32,
    class_def: &'a ClassDefinition,
) -> impl Iterator<Item = (&'a str, FeatureSource)> + 'a {
    let class_source = FeatureSource::Class(class_def.name.clone(), level);
    let subclass_source = class_level
        .subclass
        .as_deref()
        .map(|subclass| FeatureSource::Subclass(class_def.name.clone(), subclass.into(), level));

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
    character: &'a CharacterCore,
    class_idx: usize,
    level: u32,
    class_def: &'a ClassDefinition,
    features_index: FeaturesView<'a>,
) -> impl Iterator<Item = PendingFeature> + 'a {
    let class_level = &character.identity.classes[class_idx];
    let pending_keys = BTreeSet::new();
    class_level_sources(class_level, level, class_def)
        .filter(move |(name, source)| {
            let Some(feat) = features_index.get(name) else {
                return true;
            };
            if character
                .features
                .contains(&feat.name, feat.stackable, source)
            {
                return false;
            }
            // Placeholder satisfied only when an applied=true sibling fills
            // the slot. Speculative cascade pushes follow-ups as
            // applied=false, so the modal section stays mounted while the
            // user is mid-pick; once real apply lands the replacement
            // applied=true, subsequent level-ups skip the placeholder.
            let key = FeatureKey::new(feat.name.as_ref(), source.clone());
            let Some(replacement) = detect_replacement(
                &key,
                feat,
                character,
                features_index,
                &pending_keys,
                character,
            ) else {
                return true;
            };
            !character.features.iter().any(|feature| {
                feature.name == replacement && feature.source == *source && feature.applied
            })
        })
        .map(move |(name, source)| PendingFeature {
            name: name.to_string(),
            source,
            level,
            replaces: None,
        })
}

/// Collect features from a species definition.
pub fn collect_species_features<'a>(
    character: &'a CharacterCore,
    species_def: &'a SpeciesDefinition,
    features_index: FeaturesView<'a>,
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
            replaces: None,
        })
}

/// Collect features from a background definition.
pub fn collect_background_features<'a>(
    character: &'a CharacterCore,
    bg_def: &'a BackgroundDefinition,
    features_index: FeaturesView<'a>,
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
            replaces: None,
        })
}

/// Collect all unapplied features: species (if not applied), background
/// (if not applied), and class features for unapplied levels.
pub fn collect_pending_features(
    character: &CharacterCore,
    registry: &RulesRegistry,
    features_index: FeaturesView<'_>,
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

    // Iterate every 1..=level for each class. Per-feature `contains` dedup
    // inside `collect_class_features` filters out already-applied entries —
    // skipping levels by `applied.contains_level` would miss late-added
    // subclass grants whose class level was already in the applied set
    // (e.g. picking Death Domain at Cleric L3 surfaces its L1+L2 grants).
    let class_iter =
        character
            .identity
            .classes
            .iter()
            .enumerate()
            .flat_map(|(idx, class_level)| {
                let class_def = class_cache.get(class_level.class.as_str());
                (1..=class_level.level).flat_map(move |lvl| {
                    class_def.into_iter().flat_map(move |def| {
                        collect_class_features(character, idx, lvl, def, features_index)
                    })
                })
            });

    species_iter.chain(bg_iter).chain(class_iter).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use wasm_bindgen_test::wasm_bindgen_test;

    use super::*;
    use crate::{
        model::{Applied, Character, Feature, FeatureCategory, FeatureSource, IdentitySlot},
        rules::{
            ReplaceWith,
            apply::{DefinitionCaches, cascade, pending::FeatureKey},
            class::ClassDefinition,
            feature::FeatureDefinition,
            registry::make_system_feature,
        },
    };

    fn applied_feature(name: &str, source: FeatureSource) -> Feature {
        Feature {
            name: name.to_string(),
            source,
            applied: true,
            ..Feature::default()
        }
    }

    fn class_def(name: &str, l1_features: &[&str]) -> ClassDefinition {
        let levels = serde_json::json!({
            "1": { "features": l1_features },
        });
        serde_json::from_value(serde_json::json!({
            "name": name,
            "hit_die": 8,
            "levels": levels,
        }))
        .unwrap()
    }

    fn class_def_levels(name: &str, levels: &[(u32, &[&str])]) -> ClassDefinition {
        let mut levels_map = serde_json::Map::new();
        for (lvl, feats) in levels {
            levels_map.insert(lvl.to_string(), serde_json::json!({ "features": feats }));
        }
        serde_json::from_value(serde_json::json!({
            "name": name,
            "hit_die": 8,
            "levels": serde_json::Value::Object(levels_map),
        }))
        .unwrap()
    }

    fn plain_feat(name: &str) -> FeatureDefinition {
        FeatureDefinition {
            name: Box::from(name),
            stackable: false,
            category: FeatureCategory::Class,
            replace_with: ReplaceWith::None,
            spells: None,
            actions: BTreeMap::new(),
            assign: None,
            prerequisites: None,
        }
    }

    /// Like `class_def_levels`, but also defines subclasses. Each subclass
    /// entry is `(name, [(level, features)])`.
    fn class_def_with_subclasses(
        name: &str,
        levels: &[(u32, &[&str])],
        subclasses: &[(&str, &[(u32, &[&str])])],
    ) -> ClassDefinition {
        let mut levels_map = serde_json::Map::new();
        for (lvl, feats) in levels {
            levels_map.insert(lvl.to_string(), serde_json::json!({ "features": feats }));
        }
        let subclasses_json: Vec<serde_json::Value> = subclasses
            .iter()
            .map(|(sc_name, sc_levels)| {
                let mut sc_map = serde_json::Map::new();
                for (lvl, feats) in *sc_levels {
                    sc_map.insert(lvl.to_string(), serde_json::json!({ "features": feats }));
                }
                serde_json::json!({
                    "name": sc_name,
                    "levels": serde_json::Value::Object(sc_map),
                })
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "name": name,
            "hit_die": 8,
            "levels": serde_json::Value::Object(levels_map),
            "subclasses": subclasses_json,
        }))
        .unwrap()
    }

    /// Wizard L5 + Class Level placeholder → speculative cascade applies
    /// Fighter as the replacement → recompute (à la `level_up_recompute`)
    /// must surface Fighter L1 follow-ups for the modal. Reproduces the
    /// "только Class Level в модалке" symptom from the user-reported bug.
    #[wasm_bindgen_test]
    fn class_level_replacement_surfaces_new_class_l1_features() {
        // Original character: Wizard L5 with prior class features in
        // features.list at their canonical Class("Wizard", N) sources.
        let mut original = Character::default();
        original.identity.classes = vec![crate::model::ClassLevel {
            class: "Wizard".into(),
            level: 5,
            ..crate::model::ClassLevel::default()
        }];
        original.applied = Applied::default();
        for level in 1..=5u32 {
            original.applied.mark_level("Wizard", level);
            original.features.list.push(applied_feature(
                "Wizard L_n",
                FeatureSource::Class("Wizard".into(), level),
            ));
        }

        // Index: Wizard + Fighter as System(Class), plus Fighter L1 grants.
        let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index.insert(
            "Wizard".into(),
            make_system_feature("Wizard".into(), IdentitySlot::Class),
        );
        feat_index.insert(
            "Fighter".into(),
            make_system_feature("Fighter".into(), IdentitySlot::Class),
        );
        feat_index.insert(
            "Class Level".into(),
            FeatureDefinition {
                name: "Class Level".into(),
                stackable: true,
                replace_with: ReplaceWith::Category(FeatureCategory::System(IdentitySlot::Class)),
                category: FeatureCategory::Class,
                spells: None,
                actions: BTreeMap::new(),
                assign: None,
                prerequisites: None,
            },
        );
        feat_index.insert(
            "Class Proficiencies (Fighter)".into(),
            plain_feat("Class Proficiencies (Fighter)"),
        );
        feat_index.insert("Fighting Style".into(), plain_feat("Fighting Style"));
        feat_index.insert("Second Wind".into(), plain_feat("Second Wind"));

        let view = FeaturesView::from_natural(&feat_index);
        let class_index: BTreeMap<Box<str>, ClassDefinition> = [
            ("Wizard".into(), class_def("Wizard", &["Wizard L_n"])),
            (
                "Fighter".into(),
                class_def(
                    "Fighter",
                    &[
                        "Class Proficiencies (Fighter)",
                        "Fighting Style",
                        "Second Wind",
                    ],
                ),
            ),
        ]
        .into_iter()
        .collect();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };

        // Speculative cascade: clone, layer Class Level → Fighter on top.
        let mut speculative = original.core.clone();
        let target_level = 6u32;
        let placeholder = PendingFeature {
            name: "Class Level".into(),
            source: FeatureSource::User(target_level),
            level: target_level,
            replaces: None,
        };
        let inputs_for = |_: &FeatureKey| Vec::new();
        let replacement_for = |key: &FeatureKey| -> Option<String> {
            (key.name == "Class Level").then(|| "Fighter".to_string())
        };
        cascade(
            &mut speculative,
            std::slice::from_ref(&placeholder),
            view,
            caches,
            &inputs_for,
            &replacement_for,
            true,
        );

        // Sanity: cascade applied Fighter and grew identity.classes.
        assert!(
            speculative
                .identity
                .classes
                .iter()
                .any(|class_level| class_level.class == "Fighter" && class_level.level == 1),
            "speculative.identity.classes should include Fighter L1: {:?}",
            speculative.identity.classes
        );

        // Speculative cascade pushed Fighter L1 follow-ups as `applied=false`,
        // so `collect_class_features` re-emits them without any extra cleanup.
        let snapshot = speculative.clone();

        // The modal needs Fighter L1 features visible as pending. Run the
        // exact code path the recompute uses.
        let fighter_idx = snapshot
            .identity
            .classes
            .iter()
            .position(|class_level| class_level.class == "Fighter")
            .expect("Fighter in identity");
        let fighter_def = caches.classes.get("Fighter").expect("Fighter def");
        let pending: Vec<PendingFeature> =
            collect_class_features(&snapshot, fighter_idx, 1, fighter_def, view).collect();

        let names: Vec<&str> = pending.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"Class Proficiencies (Fighter)"),
            "expected Fighter L1 grants in pending after class swap, got {names:?}"
        );
    }

    /// Cleric L1 already on the sheet → user picks "Cleric" again as the
    /// L2 replacement → cascade must bump CLASS.Cleric.LEVEL to 2 and emit
    /// Cleric L2 follow-ups. Reproduces the L2 level-up regression.
    #[wasm_bindgen_test]
    fn level_up_existing_class_bumps_class_level() {
        let mut original = Character::default();
        original.identity.classes = vec![ClassLevel {
            class: "Cleric".into(),
            level: 1,
            ..ClassLevel::default()
        }];
        original.applied = Applied::default();
        original.applied.mark_level("Cleric", 1);
        // Mirror the post-L1 features.list shape:
        // - System(Class) marker for Cleric at User(1)
        // - L1 grant under Class("Cleric", 1)
        original.features.list.push(Feature {
            name: "Cleric".into(),
            source: FeatureSource::User(1),
            applied: true,
            category: FeatureCategory::System(IdentitySlot::Class),
            ..Feature::default()
        });
        original.features.list.push(applied_feature(
            "Class Proficiencies (Cleric)",
            FeatureSource::Class("Cleric".into(), 1),
        ));

        let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index.insert(
            "Cleric".into(),
            make_system_feature("Cleric".into(), IdentitySlot::Class),
        );
        feat_index.insert(
            "Class Level".into(),
            FeatureDefinition {
                name: "Class Level".into(),
                stackable: true,
                replace_with: ReplaceWith::Category(FeatureCategory::System(IdentitySlot::Class)),
                category: FeatureCategory::Class,
                spells: None,
                actions: BTreeMap::new(),
                assign: None,
                prerequisites: None,
            },
        );
        feat_index.insert(
            "Class Proficiencies (Cleric)".into(),
            plain_feat("Class Proficiencies (Cleric)"),
        );
        feat_index.insert("Channel Divinity".into(), plain_feat("Channel Divinity"));

        let view = FeaturesView::from_natural(&feat_index);
        let class_index: BTreeMap<Box<str>, ClassDefinition> = [(
            "Cleric".into(),
            class_def_levels(
                "Cleric",
                &[
                    (1, &["Class Proficiencies (Cleric)"]),
                    (2, &["Channel Divinity"]),
                ],
            ),
        )]
        .into_iter()
        .collect();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };

        let mut character = original.core.clone();
        let target_level = 2u32;
        let placeholder = PendingFeature {
            name: "Class Level".into(),
            source: FeatureSource::User(target_level),
            level: target_level,
            replaces: None,
        };
        let inputs_for = |_: &FeatureKey| Vec::new();
        let replacement_for = |key: &FeatureKey| -> Option<String> {
            (key.name == "Class Level").then(|| "Cleric".to_string())
        };
        cascade(
            &mut character,
            std::slice::from_ref(&placeholder),
            view,
            caches,
            &inputs_for,
            &replacement_for,
            false,
        );

        // The bug: after cascade, CLASS.Cleric.LEVEL is still 1.
        let cleric_level = character
            .identity
            .classes
            .iter()
            .find(|class_level| class_level.class == "Cleric")
            .map(|class_level| class_level.level)
            .unwrap_or(0);
        assert_eq!(
            cleric_level, 2,
            "expected Cleric L2 after cascade, got {cleric_level}; classes={:?}",
            character.identity.classes
        );

        // L2 grant should have landed too.
        assert!(
            character
                .features
                .list
                .iter()
                .any(|feature| feature.name == "Channel Divinity" && feature.applied),
            "expected Channel Divinity (Cleric L2 grant) applied, got: {:?}",
            character
                .features
                .list
                .iter()
                .map(|feature| (&feature.name, &feature.source, feature.applied))
                .collect::<Vec<_>>()
        );
    }

    /// Empty char → first level-up seeds the `Class Level` placeholder
    /// using `class_level_sum() + 1 = 1`, so the marker lands at `User(1)`.
    /// Second click computes `1 + 1 = 2` for the next placeholder — distinct
    /// `User(2)` source so `contains` returns false and cascade applies the
    /// level bump.
    #[wasm_bindgen_test]
    fn level_up_collides_at_user_2_after_first_apply() {
        // Post-first-apply state: Cleric L1, marker at User(1).
        let mut character = Character::default().core;
        character.identity.classes = vec![ClassLevel {
            class: "Cleric".into(),
            level: 1,
            ..ClassLevel::default()
        }];
        character.applied = Applied::default();
        character.applied.mark_level("Cleric", 1);
        character.features.list.push(Feature {
            name: "Cleric".into(),
            source: FeatureSource::User(1),
            applied: true,
            category: FeatureCategory::System(IdentitySlot::Class),
            ..Feature::default()
        });

        let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index.insert(
            "Cleric".into(),
            make_system_feature("Cleric".into(), IdentitySlot::Class),
        );
        feat_index.insert(
            "Class Level".into(),
            FeatureDefinition {
                name: "Class Level".into(),
                stackable: true,
                replace_with: ReplaceWith::Category(FeatureCategory::System(IdentitySlot::Class)),
                category: FeatureCategory::Class,
                spells: None,
                actions: BTreeMap::new(),
                assign: None,
                prerequisites: None,
            },
        );
        feat_index.insert("Channel Divinity".into(), plain_feat("Channel Divinity"));

        let view = FeaturesView::from_natural(&feat_index);
        let class_index: BTreeMap<Box<str>, ClassDefinition> = [(
            "Cleric".into(),
            class_def_levels("Cleric", &[(1, &[]), (2, &["Channel Divinity"])]),
        )]
        .into_iter()
        .collect();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };

        // Production target_level = class_level_sum() + 1 = 1+1 = 2.
        let target_level = 2u32;
        let placeholder = PendingFeature {
            name: "Class Level".into(),
            source: FeatureSource::User(target_level),
            level: target_level,
            replaces: None,
        };
        let inputs_for = |_: &FeatureKey| Vec::new();
        let replacement_for = |key: &FeatureKey| -> Option<String> {
            (key.name == "Class Level").then(|| "Cleric".to_string())
        };
        cascade(
            &mut character,
            std::slice::from_ref(&placeholder),
            view,
            caches,
            &inputs_for,
            &replacement_for,
            false,
        );

        let cleric_level = character
            .identity
            .classes
            .iter()
            .find(|class_level| class_level.class == "Cleric")
            .map(|class_level| class_level.level)
            .unwrap_or(0);
        assert_eq!(
            cleric_level,
            2,
            "expected Cleric L2 after second level-up, got {cleric_level}; \
             features={:?}",
            character
                .features
                .list
                .iter()
                .map(|feature| (&feature.name, &feature.source, feature.applied))
                .collect::<Vec<_>>()
        );
    }

    /// Bug #2 regression: at L3 levelup, after the user picks a subclass,
    /// the speculative cascade must emit the subclass's L1..=current_level
    /// features so the modal shows them as new sections. Reproduces the
    /// "Death Domain L1 features missing from modal" symptom (Bonus
    /// Proficiency, Reaper).
    ///
    /// Setup: Cleric L2 already on sheet. Modal pending = [Class Level,
    /// Subclass]. User picks Cleric (→L3) and Death Domain. The
    /// speculative cascade with both replacements must surface Death
    /// Domain's L1 grants (Bonus Proficiency, Reaper) and L2 grant
    /// (Channel Divinity: Touch of Death) as `applied=false` rows.
    #[wasm_bindgen_test]
    fn subclass_pick_surfaces_subclass_l1_features() {
        let mut original = Character::default();
        original.identity.classes = vec![ClassLevel {
            class: "Cleric".into(),
            level: 2,
            ..ClassLevel::default()
        }];
        original.applied = Applied::default();
        original.applied.mark_level("Cleric", 1);
        original.applied.mark_level("Cleric", 2);
        original.features.list.push(Feature {
            name: "Cleric".into(),
            source: FeatureSource::User(1),
            applied: true,
            category: FeatureCategory::System(IdentitySlot::Class),
            ..Feature::default()
        });
        original.features.list.push(Feature {
            name: "Cleric".into(),
            source: FeatureSource::User(2),
            applied: true,
            category: FeatureCategory::System(IdentitySlot::Class),
            ..Feature::default()
        });

        let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index.insert(
            "Cleric".into(),
            make_system_feature("Cleric".into(), IdentitySlot::Class),
        );
        feat_index.insert(
            "Death Domain".into(),
            make_system_feature("Death Domain".into(), IdentitySlot::Subclass),
        );
        feat_index.insert(
            "Class Level".into(),
            FeatureDefinition {
                name: "Class Level".into(),
                stackable: true,
                replace_with: ReplaceWith::Category(FeatureCategory::System(IdentitySlot::Class)),
                category: FeatureCategory::Class,
                spells: None,
                actions: BTreeMap::new(),
                assign: None,
                prerequisites: None,
            },
        );
        feat_index.insert(
            "Subclass".into(),
            FeatureDefinition {
                name: "Subclass".into(),
                stackable: false,
                replace_with: ReplaceWith::Category(FeatureCategory::System(
                    IdentitySlot::Subclass,
                )),
                category: FeatureCategory::Class,
                spells: None,
                actions: BTreeMap::new(),
                assign: None,
                prerequisites: None,
            },
        );
        feat_index.insert("Bonus Proficiency".into(), plain_feat("Bonus Proficiency"));
        feat_index.insert("Reaper".into(), plain_feat("Reaper"));
        feat_index.insert(
            "Channel Divinity: Touch of Death".into(),
            plain_feat("Channel Divinity: Touch of Death"),
        );

        let view = FeaturesView::from_natural(&feat_index);
        let class_index: BTreeMap<Box<str>, ClassDefinition> = [(
            "Cleric".into(),
            class_def_with_subclasses(
                "Cleric",
                &[
                    (1u32, &[] as &[&str]),
                    (2u32, &[] as &[&str]),
                    (3u32, &["Subclass"] as &[&str]),
                ],
                &[(
                    "Death Domain",
                    &[
                        (1u32, &["Bonus Proficiency", "Reaper"] as &[&str]),
                        (2u32, &["Channel Divinity: Touch of Death"] as &[&str]),
                        (3u32, &[] as &[&str]),
                    ],
                )],
            ),
        )]
        .into_iter()
        .collect();
        let species_cache: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_cache: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_index,
            species: &species_cache,
            backgrounds: &bg_cache,
        };

        // Mimic the args-modal speculative cascade with both replacements
        // supplied: Class Level → Cleric, Subclass → Death Domain.
        let mut speculative = original.core.clone();
        let target_level = 3u32;
        let placeholder = PendingFeature {
            name: "Class Level".into(),
            source: FeatureSource::User(target_level),
            level: target_level,
            replaces: None,
        };
        let inputs_for = |_: &FeatureKey| Vec::new();
        let replacement_for = |key: &FeatureKey| -> Option<String> {
            match key.name.as_str() {
                "Class Level" => Some("Cleric".to_string()),
                "Subclass" => Some("Death Domain".to_string()),
                _ => None,
            }
        };
        cascade(
            &mut speculative,
            std::slice::from_ref(&placeholder),
            view,
            caches,
            &inputs_for,
            &replacement_for,
            true,
        );

        // Sanity: subclass landed on the Cleric class entry.
        let cleric = speculative
            .identity
            .classes
            .iter()
            .find(|class_level| class_level.class == "Cleric")
            .expect("Cleric in identity");
        assert_eq!(
            cleric.subclass.as_deref(),
            Some("Death Domain"),
            "expected Cleric.subclass = Death Domain after cascade, got {:?}",
            cleric.subclass
        );

        // Death Domain L1+L2 features must be present in features.list as
        // applied=false placeholders for the modal (speculative=true).
        let feature_names: Vec<&str> = speculative
            .features
            .list
            .iter()
            .map(|feature| feature.name.as_str())
            .collect();
        assert!(
            feature_names.contains(&"Bonus Proficiency"),
            "expected Bonus Proficiency (Death Domain L1) in features after \
             speculative cascade, got {feature_names:?}"
        );
        assert!(
            feature_names.contains(&"Reaper"),
            "expected Reaper (Death Domain L1) in features after \
             speculative cascade, got {feature_names:?}"
        );
        assert!(
            feature_names.contains(&"Channel Divinity: Touch of Death"),
            "expected Channel Divinity: Touch of Death (Death Domain L2) in \
             features after speculative cascade, got {feature_names:?}"
        );
    }

    /// Bug #2 root-cause test: directly exercises `collect_class_features`
    /// at L1 with a subclass set. The character's `applied.levels["Cleric"]`
    /// already contains 1 (e.g. char was at Cleric L2 before subclass pick),
    /// but the subclass L1 grants haven't been applied yet — they must
    /// still be emitted. Pre-fix, `collect_pending_features` skipped this
    /// level entirely because the class level was in the applied set.
    #[wasm_bindgen_test]
    fn collect_class_features_at_applied_level_emits_new_subclass_grants() {
        let mut character = Character::default().core;
        character.identity.classes = vec![ClassLevel {
            class: "Cleric".into(),
            level: 3,
            subclass: Some("Death Domain".into()),
            ..ClassLevel::default()
        }];
        character.applied = Applied::default();
        character.applied.mark_level("Cleric", 1);
        character.applied.mark_level("Cleric", 2);
        character.applied.mark_level("Cleric", 3);
        // No subclass features applied yet — the user just picked the
        // subclass; the class-level apply has already landed.

        let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index.insert("Bonus Proficiency".into(), plain_feat("Bonus Proficiency"));
        feat_index.insert("Reaper".into(), plain_feat("Reaper"));
        let view = FeaturesView::from_natural(&feat_index);

        let class_def = class_def_with_subclasses(
            "Cleric",
            &[(1u32, &[] as &[&str])],
            &[(
                "Death Domain",
                &[(1u32, &["Bonus Proficiency", "Reaper"] as &[&str])],
            )],
        );

        let pending: Vec<PendingFeature> =
            collect_class_features(&character, 0, 1, &class_def, view).collect();
        let names: Vec<&str> = pending.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"Bonus Proficiency"),
            "subclass L1 features must be emitted even when class level is \
             marked applied; got {names:?}"
        );
        assert!(
            names.contains(&"Reaper"),
            "subclass L1 features must be emitted even when class level is \
             marked applied; got {names:?}"
        );
    }
}
