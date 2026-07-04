//! Tier-B reactive smoke tests. Exercise `RulesRegistry::for_test`
//! harness — confirm the in-memory registry returns provided definitions
//! through the same `with_features_index_untracked` / `with_definitions`
//! APIs the apply pipeline uses in production.

#![cfg(target_arch = "wasm32")]

use std::collections::BTreeMap;

use dnd_pc::{
    model::{
        Ability, CharacterCore, ClassLevel, Feature, FeatureCategory, FeatureData, FeatureSource,
        SpellData, SpellSlotPool,
    },
    rules::{FeatureDefinition, FeaturesIndex, ReplaceWith, RulesRegistry},
};
use leptos::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn for_test_registry_exposes_features() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.set();

    let mut features = BTreeMap::<Box<str>, FeatureDefinition>::new();
    features.insert(
        "Test Feature".into(),
        FeatureDefinition {
            name: "Test Feature".into(),
            stackable: false,
            category: Default::default(),
            replace_with: Default::default(),
            spells: None,
            actions: BTreeMap::new(),
            assign: None,
            prerequisites: None,
        },
    );
    let registry = RulesRegistry::for_test(
        FeaturesIndex(features),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    registry.await_ready().await;

    let names: Vec<String> = registry
        .with_features_index_untracked(|view| view.iter().map(|(k, _)| k.to_string()).collect());
    assert!(
        names.contains(&"Test Feature".to_string()),
        "for_test registry should expose injected feature, got {names:?}"
    );
}

/// Mirrors "Alaric Kestrel": single Warlock, Pact Magic in the Pact pool.
fn warlock_core(level: u32) -> CharacterCore {
    let mut core = CharacterCore::default();
    core.identity.classes.push(ClassLevel {
        class: "Warlock".into(),
        level,
        hit_die_sides: 8,
        ..ClassLevel::default()
    });
    core.identity.species = "Khoravar".into();
    core.identity.background = "Criminal".into();
    core.features.list.push(Feature {
        name: "Pact Magic".into(),
        source: FeatureSource::Class("Warlock".into(), 1),
        applied: true,
        ..Feature::default()
    });
    core.features.insert(
        "Pact Magic".into(),
        FeatureData {
            spells: Some(SpellData {
                casting_ability: Ability::Charisma,
                caster_coef: 1,
                pool: SpellSlotPool::Pact,
                spells: Vec::new(),
                known: None,
            }),
            ..FeatureData::default()
        },
    );
    core
}

#[wasm_bindgen_test]
async fn replacement_candidates_evaluate_given_character() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.set();

    let mut features = BTreeMap::<Box<str>, FeatureDefinition>::new();
    features.insert(
        "Elemental Adept (Fire)".into(),
        FeatureDefinition {
            name: "Elemental Adept (Fire)".into(),
            stackable: false,
            category: FeatureCategory::General,
            replace_with: ReplaceWith::None,
            spells: None,
            actions: BTreeMap::new(),
            assign: None,
            prerequisites: Some("LEVEL >= 4 and CASTER.LEVEL >= 1".parse().unwrap()),
        },
    );
    let registry = RulesRegistry::for_test(
        FeaturesIndex(features),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    registry.await_ready().await;

    // Speculative level-4 snapshot (what the picker must evaluate against).
    let names = registry.replacement_candidates(&warlock_core(4), &ReplaceWith::Any, None);
    assert!(
        names.iter().any(|name| &**name == "Elemental Adept (Fire)"),
        "level-4 pact warlock must qualify, got {names:?}"
    );

    // Pre-level-up character: the level gate must still hold.
    let names = registry.replacement_candidates(&warlock_core(3), &ReplaceWith::Any, None);
    assert!(
        !names.iter().any(|name| &**name == "Elemental Adept (Fire)"),
        "level-3 warlock must not qualify, got {names:?}"
    );
}
