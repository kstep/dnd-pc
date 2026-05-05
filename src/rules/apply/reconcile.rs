use std::collections::{BTreeMap, VecDeque};

use leptos::prelude::ReadUntracked;

use crate::{
    model::{Character, CharacterIdentity, FeatureCategory, FeatureSource},
    rules::{
        DefinitionStore, RulesRegistry, apply::collect::class_level_sources,
        background::BackgroundDefinition, class::ClassDefinition, species::SpeciesDefinition,
    },
};

// Local grouping for reconcile's two pure entry points — keeps the signature
// tight without committing to a project-wide `Caches` abstraction. Promote to
// `src/rules/` if a second call site ever needs the same bundle.
pub struct DefinitionCaches<'a> {
    pub classes: &'a BTreeMap<Box<str>, ClassDefinition>,
    pub species: &'a BTreeMap<Box<str>, SpeciesDefinition>,
    pub backgrounds: &'a BTreeMap<Box<str>, BackgroundDefinition>,
}

/// Rewrite `User(_)` sources on features whose names match a canonical slot
/// granted by the character's identity. Preserves `inputs`, `applied`,
/// `label`, `description`. Genuine user-added features (no matching identity
/// slot) are left alone.
pub fn reconcile_user_feature_sources(character: &mut Character, registry: &RulesRegistry) {
    let class_cache = registry.classes().cache().read_untracked();
    let species_cache = registry.species().cache().read_untracked();
    let backgrounds_cache = registry.backgrounds().cache().read_untracked();
    reconcile_with_defs(
        character,
        DefinitionCaches {
            classes: &class_cache,
            species: &species_cache,
            backgrounds: &backgrounds_cache,
        },
    );
}

// Three sequential passes over features.list are intentional: (1) early-exit
// guard skips slot construction when nothing to reconcile, (2) non-User pass
// vacates claimed slots, (3) User pass drains the remaining slots. Folding
// the guard + pre-subtract requires splitting `character` borrows around a
// lazy-init closure; the borrow-checker gymnastics aren't worth saving
// ~80 comparisons on a button-click path.
pub fn reconcile_with_defs(character: &mut Character, caches: DefinitionCaches<'_>) {
    if !character
        .features
        .iter()
        .any(|feature| feature.source.is_user())
    {
        return;
    }
    let mut slots = build_canonical_slots(&character.identity, caches);

    // Slots already claimed by non-User features are removed from the queue so
    // reconcile never hands the same canonical source to two different entries.
    for feature in character.features.iter() {
        if feature.source.is_user() {
            continue;
        }
        if let Some(queue) = slots.get_mut(feature.name.as_str()) {
            queue.retain(|source| *source != feature.source);
        }
    }

    for feature in character.features.list.iter_mut() {
        if !feature.source.is_user() {
            continue;
        }
        // System(_) markers live on `User(N)` by design — their name matches
        // an identity-slot owner (class / subclass / species / background)
        // but the `User(N)` source is canonical for the level-up sequence.
        // Rewriting them to a Class/Subclass/etc. slot here would corrupt
        // the marker and force the rebuild plan-builder to re-emit them.
        if matches!(feature.category, FeatureCategory::System(_)) {
            continue;
        }
        if let Some(queue) = slots.get_mut(feature.name.as_str())
            && let Some(canonical) = queue.pop_front()
        {
            feature.source = canonical;
        }
    }
}

// Reconcile relies on FIFO ordering of the queue: stackable features like ASI
// appear at multiple class levels and each instance must bind to the earliest
// unclaimed slot so stored inputs align with increasing level order.
pub fn build_canonical_slots(
    identity: &CharacterIdentity,
    caches: DefinitionCaches<'_>,
) -> BTreeMap<Box<str>, VecDeque<FeatureSource>> {
    let mut slots: BTreeMap<Box<str>, VecDeque<FeatureSource>> = BTreeMap::new();

    if !identity.species.is_empty()
        && let Some(def) = caches.species.get(identity.species.as_str())
    {
        let source = FeatureSource::Species(identity.species.as_str().into());
        def.features.iter().for_each(|name| {
            slots
                .entry(name.as_str().into())
                .or_default()
                .push_back(source.clone());
        });
    }
    if !identity.background.is_empty()
        && let Some(def) = caches.backgrounds.get(identity.background.as_str())
    {
        let source = FeatureSource::Background(identity.background.as_str().into());
        def.features.iter().for_each(|name| {
            slots
                .entry(name.as_str().into())
                .or_default()
                .push_back(source.clone());
        });
    }

    for class_level in &identity.classes {
        if class_level.class.is_empty() {
            continue;
        }
        let Some(class_def) = caches.classes.get(class_level.class.as_str()) else {
            continue;
        };
        for level in 1..=class_level.level {
            for (name, source) in class_level_sources(class_level, level, class_def) {
                slots.entry(name.into()).or_default().push_back(source);
            }
        }
    }

    slots
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::{
        model::{AssignInputs, ClassLevel, Feature},
        rules::class::ClassDefinition,
    };

    fn cache<T>(entries: Vec<(&str, T)>) -> BTreeMap<Box<str>, T> {
        entries
            .into_iter()
            .map(|(k, v)| (Box::from(k), v))
            .collect()
    }

    fn rogue_def() -> ClassDefinition {
        serde_json::from_value(serde_json::json!({
            "name": "Rogue",
            "hit_die": 8,
            "levels": {
                "1": { "features": ["Class Proficiencies (Rogue)", "Expertise", "Sneak Attack"] },
                "2": { "features": ["Cunning Action"] },
                "3": { "features": ["Steady Aim"] },
                "4": { "features": ["Ability Score Improvement"] },
                "5": { "features": ["Cunning Strike", "Uncanny Dodge"] },
                "7": { "features": ["Evasion"] },
                "8": { "features": ["Ability Score Improvement"] }
            },
            "subclasses": [
                {
                    "name": "Thief",
                    "levels": {
                        "3": { "features": ["Fast Hands", "Second-Story Work"] },
                        "9": { "features": ["Supreme Sneak"] }
                    }
                }
            ]
        }))
        .unwrap()
    }

    fn fighter_def() -> ClassDefinition {
        serde_json::from_value(serde_json::json!({
            "name": "Fighter",
            "hit_die": 10,
            "levels": {
                "1": { "features": ["Class Proficiencies (Fighter)", "Second Wind"] },
                "2": { "features": ["Action Surge"] },
                "4": { "features": ["Ability Score Improvement"] },
                "5": { "features": ["Extra Attack"] },
                "6": { "features": ["Ability Score Improvement"] }
            },
            "subclasses": []
        }))
        .unwrap()
    }

    fn rogue6_thief() -> Character {
        let mut character = Character::default();
        character.identity.classes = vec![ClassLevel {
            class: "Rogue".into(),
            subclass: Some("Thief".into()),
            level: 6,
            ..ClassLevel::default()
        }];
        character.identity.species = "Rock Gnome".into();
        character.identity.background = "Criminal".into();
        character
    }

    type OwnedCaches = (
        BTreeMap<Box<str>, ClassDefinition>,
        BTreeMap<Box<str>, SpeciesDefinition>,
        BTreeMap<Box<str>, BackgroundDefinition>,
    );

    fn default_caches() -> OwnedCaches {
        let classes = cache(vec![("Rogue", rogue_def())]);
        let species = cache(vec![(
            "Rock Gnome",
            serde_json::from_value::<SpeciesDefinition>(serde_json::json!({
                "name": "Rock Gnome",
                "features": ["Gnome Cunning", "Artificer's Lore"]
            }))
            .unwrap(),
        )]);
        let backgrounds = cache(vec![(
            "Criminal",
            serde_json::from_value::<BackgroundDefinition>(serde_json::json!({
                "name": "Criminal",
                "features": ["Criminal Contact"]
            }))
            .unwrap(),
        )]);
        (classes, species, backgrounds)
    }

    fn feature(name: &str, source: FeatureSource) -> Feature {
        Feature {
            name: name.to_string(),
            source,
            applied: true,
            ..Feature::default()
        }
    }

    fn run_reconcile(character: &mut Character, caches: &OwnedCaches) {
        let (classes, species, backgrounds) = caches;
        reconcile_with_defs(
            character,
            DefinitionCaches {
                classes,
                species,
                backgrounds,
            },
        );
    }

    #[wasm_bindgen_test]
    fn reconcile_reassigns_class_feature() {
        let mut original = rogue6_thief();
        original.features.list.push(Feature {
            applied: false,
            ..feature("Class Proficiencies (Rogue)", FeatureSource::User(0))
        });
        run_reconcile(&mut original, &default_caches());

        assert_eq!(
            original.features.list[0].source,
            FeatureSource::Class("Rogue".into(), 1)
        );
        assert!(!original.features.list[0].applied);
    }

    #[wasm_bindgen_test]
    fn reconcile_reassigns_subclass_feature() {
        let mut original = rogue6_thief();
        original
            .features
            .list
            .push(feature("Fast Hands", FeatureSource::User(0)));
        run_reconcile(&mut original, &default_caches());

        assert_eq!(
            original.features.list[0].source,
            FeatureSource::Subclass("Rogue".into(), "Thief".into(), 3)
        );
    }

    #[wasm_bindgen_test]
    fn reconcile_reassigns_species_and_background_features() {
        let mut original = rogue6_thief();
        original
            .features
            .list
            .push(feature("Gnome Cunning", FeatureSource::User(0)));
        original
            .features
            .list
            .push(feature("Criminal Contact", FeatureSource::User(0)));
        run_reconcile(&mut original, &default_caches());

        assert_eq!(
            original.features.list[0].source,
            FeatureSource::Species("Rock Gnome".into())
        );
        assert_eq!(
            original.features.list[1].source,
            FeatureSource::Background("Criminal".into())
        );
    }

    #[wasm_bindgen_test]
    fn reconcile_assigns_stackable_multiple_slots() {
        let mut original = rogue6_thief();
        original.identity.classes[0].level = 8;
        original
            .features
            .list
            .push(feature("Ability Score Improvement", FeatureSource::User(0)));
        original
            .features
            .list
            .push(feature("Ability Score Improvement", FeatureSource::User(0)));
        run_reconcile(&mut original, &default_caches());

        assert_eq!(
            original.features.list[0].source,
            FeatureSource::Class("Rogue".into(), 4)
        );
        assert_eq!(
            original.features.list[1].source,
            FeatureSource::Class("Rogue".into(), 8)
        );
    }

    #[wasm_bindgen_test]
    fn reconcile_leaves_genuine_user_feature_alone() {
        let mut original = rogue6_thief();
        original
            .features
            .list
            .push(feature("Lucky", FeatureSource::User(4)));
        run_reconcile(&mut original, &default_caches());

        assert_eq!(original.features.list[0].source, FeatureSource::User(4));
    }

    #[wasm_bindgen_test]
    fn reconcile_preserves_inputs_and_applied() {
        let mut original = rogue6_thief();
        let stored_inputs = vec![AssignInputs {
            args: vec![1, 2, 3],
            dice: Default::default(),
        }];
        original.features.list.push(Feature {
            applied: true,
            inputs: stored_inputs.clone(),
            ..feature("Class Proficiencies (Rogue)", FeatureSource::User(0))
        });
        run_reconcile(&mut original, &default_caches());
        let feat = &original.features.list[0];
        assert_eq!(feat.source, FeatureSource::Class("Rogue".into(), 1));
        assert!(feat.applied);
        assert_eq!(feat.inputs, stored_inputs);
    }

    #[wasm_bindgen_test]
    fn reconcile_leaves_unmatched_user_feature_as_user() {
        let mut original = rogue6_thief();
        original
            .features
            .list
            .push(feature("Generation: User-Defined", FeatureSource::User(0)));
        run_reconcile(&mut original, &default_caches());

        assert_eq!(original.features.list[0].source, FeatureSource::User(0));
    }

    #[wasm_bindgen_test]
    fn canonical_slots_skip_class_levels_above_applied() {
        let character = rogue6_thief();
        let (classes, species, backgrounds) = default_caches();
        let slots = build_canonical_slots(
            &character.identity,
            DefinitionCaches {
                classes: &classes,
                species: &species,
                backgrounds: &backgrounds,
            },
        );

        assert!(!slots.contains_key("Evasion"));
        assert!(!slots.contains_key("Supreme Sneak"));
        assert!(slots.contains_key("Cunning Action"));
    }

    #[wasm_bindgen_test]
    fn reconcile_noop_when_no_user_features() {
        let mut original = rogue6_thief();
        original.features.list.push(feature(
            "Expertise",
            FeatureSource::Class("Rogue".into(), 1),
        ));
        let before = original.features.list.clone();
        run_reconcile(&mut original, &default_caches());
        assert_eq!(original.features.list.as_slice(), before.as_slice());
    }

    #[wasm_bindgen_test]
    fn reconcile_skips_slots_already_claimed_by_canonical() {
        let mut original = rogue6_thief();
        original.identity.classes[0].level = 8;
        original.features.list.push(feature(
            "Ability Score Improvement",
            FeatureSource::Class("Rogue".into(), 4),
        ));
        original
            .features
            .list
            .push(feature("Ability Score Improvement", FeatureSource::User(0)));
        run_reconcile(&mut original, &default_caches());

        assert_eq!(
            original.features.list[0].source,
            FeatureSource::Class("Rogue".into(), 4),
        );
        assert_eq!(
            original.features.list[1].source,
            FeatureSource::Class("Rogue".into(), 8),
        );
    }

    // Multiclass overlap: two classes both granting ASI must yield canonical
    // slots in class order × increasing level order.
    #[wasm_bindgen_test]
    fn reconcile_multiclass_asi_fifo_ordering() {
        let mut character = Character::default();
        character.identity.classes = vec![
            ClassLevel {
                class: "Fighter".into(),
                level: 4,
                ..ClassLevel::default()
            },
            ClassLevel {
                class: "Rogue".into(),
                subclass: Some("Thief".into()),
                level: 4,
                ..ClassLevel::default()
            },
        ];
        character
            .features
            .list
            .push(feature("Ability Score Improvement", FeatureSource::User(0)));
        character
            .features
            .list
            .push(feature("Ability Score Improvement", FeatureSource::User(0)));

        let classes = cache(vec![("Fighter", fighter_def()), ("Rogue", rogue_def())]);
        let species = BTreeMap::new();
        let backgrounds = BTreeMap::new();
        let caches = (classes, species, backgrounds);
        run_reconcile(&mut character, &caches);

        assert_eq!(
            character.features.list[0].source,
            FeatureSource::Class("Fighter".into(), 4),
        );
        assert_eq!(
            character.features.list[1].source,
            FeatureSource::Class("Rogue".into(), 4),
        );
    }

    #[wasm_bindgen_test]
    fn reconcile_skips_system_class_marker() {
        // System(Class) markers live on User(N) by design. Reconcile must
        // leave them alone or the rebuild plan-builder would re-emit them
        // and we'd end up with duplicate Class slots.
        use crate::model::IdentitySlot;
        let mut character = rogue6_thief();
        character.features.list.push(Feature {
            name: "Fighter".into(),
            category: FeatureCategory::System(IdentitySlot::Class),
            source: FeatureSource::User(3),
            applied: true,
            ..Feature::default()
        });
        let owned = (
            cache(vec![("Rogue", rogue_def()), ("Fighter", fighter_def())]),
            cache(vec![]),
            cache(vec![]),
        );
        run_reconcile(&mut character, &owned);

        let marker = character
            .features
            .list
            .iter()
            .find(|feature| feature.name == "Fighter")
            .expect("marker preserved");
        assert_eq!(marker.source, FeatureSource::User(3));
        assert!(matches!(
            marker.category,
            FeatureCategory::System(IdentitySlot::Class)
        ));
    }

    // Missing definitions in cache: reconcile must keep User(_) untouched when
    // class/species/bg definitions aren't available.
    #[wasm_bindgen_test]
    fn reconcile_preserves_user_when_class_def_missing() {
        let mut original = rogue6_thief();
        original.features.list.push(feature(
            "Class Proficiencies (Rogue)",
            FeatureSource::User(0),
        ));

        let empty: OwnedCaches = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
        run_reconcile(&mut original, &empty);

        assert_eq!(original.features.list[0].source, FeatureSource::User(0));
    }
}
