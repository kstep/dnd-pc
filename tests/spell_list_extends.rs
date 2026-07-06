//! Regression: a feature whose `spells.extends` names a host caster widens
//! the host's learnable list (Tasha expanded spell lists) without getting a
//! spellcasting block of its own.

#![cfg(target_arch = "wasm32")]

use std::collections::BTreeMap;

use dnd_pc::{
    components::panels::spellcasting::resolve_feature_spell_list,
    model::{Feature, FeatureSource, Features},
    rules::{FeatureDefinition, FeaturesIndex, RulesRegistry, SpellsIndex},
};
use leptos::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

fn feature_row(name: &str) -> Feature {
    Feature {
        name: name.into(),
        applied: true,
        source: FeatureSource::Class("Warlock".into(), 1),
        ..Feature::default()
    }
}

#[wasm_bindgen_test]
async fn extenders_widen_host_learn_list() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.set();

    let mut defs = BTreeMap::<Box<str>, FeatureDefinition>::new();
    for value in [
        serde_json::json!({
            "name": "Pact Magic",
            "spells": {"list": [{"name": "Magic Missile"}]},
        }),
        serde_json::json!({
            "name": "Expanded Spell List (Dao)",
            "spells": {
                "extends": "Pact Magic",
                // Magic Missile duplicates the host list on purpose.
                "list": [{"name": "Sanctuary"}, {"name": "Magic Missile"}],
            },
        }),
    ] {
        let def: FeatureDefinition = serde_json::from_value(value).expect("feature def");
        defs.insert(def.name.clone(), def);
    }
    let spells: SpellsIndex = serde_json::from_value(serde_json::json!([
        {"name": "Magic Missile", "level": 1},
        {"name": "Sanctuary", "level": 1},
    ]))
    .expect("spells index");

    let registry = RulesRegistry::for_test(
        FeaturesIndex(defs),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    )
    .with_spells(spells);
    registry.await_ready().await;

    let mut features = Features::default();
    features.push(feature_row("Pact Magic"));
    features.push(feature_row("Expanded Spell List (Dao)"));

    let buckets = resolve_feature_spell_list(&registry, "Pact Magic", &features);
    let names: Vec<&str> = buckets[1].iter().map(|opt| opt.name.as_str()).collect();
    assert!(
        names.contains(&"Magic Missile"),
        "host list stays: {names:?}"
    );
    assert!(
        names.contains(&"Sanctuary"),
        "extender list merges in: {names:?}"
    );
    assert_eq!(
        names
            .iter()
            .filter(|name| **name == "Magic Missile")
            .count(),
        1,
        "duplicates collapse: {names:?}"
    );

    let mut host_only = Features::default();
    host_only.push(feature_row("Pact Magic"));
    let buckets = resolve_feature_spell_list(&registry, "Pact Magic", &host_only);
    let names: Vec<&str> = buckets[1].iter().map(|opt| opt.name.as_str()).collect();
    assert!(
        !names.contains(&"Sanctuary"),
        "no extender on the character — no merge: {names:?}"
    );
}
