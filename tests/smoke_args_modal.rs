//! Tier-B reactive smoke tests. Exercise `RulesRegistry::for_test`
//! harness — confirm the in-memory registry returns provided definitions
//! through the same `with_features_index_untracked` / `with_definitions`
//! APIs the apply pipeline uses in production.

#![cfg(target_arch = "wasm32")]

use std::collections::BTreeMap;

use dnd_pc::rules::{FeatureDefinition, FeaturesIndex, RulesRegistry};
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
