//! Regression: a non-initial class grants only its multiclass
//! proficiency set (no saves, limited armor), but does set its hit die.

#![cfg(target_arch = "wasm32")]

use std::collections::BTreeMap;

use dnd_pc::{
    model::{Ability, CharacterCore, ClassLevel, Proficiency},
    rules::{
        ClassDefinition, FeatureDefinition, FeaturesIndex, RulesRegistry,
        apply::{cascade, collect_pending_features},
    },
};
use leptos::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

fn feature(value: serde_json::Value) -> (Box<str>, FeatureDefinition) {
    let def: FeatureDefinition = serde_json::from_value(value).expect("feature def");
    (def.name.clone(), def)
}

#[wasm_bindgen_test]
async fn multiclass_grants_limited_proficiencies() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.set();

    let mut features = BTreeMap::<Box<str>, FeatureDefinition>::new();
    // Verbatim copies of the real data rows (public/data/features.json).
    for value in [
        serde_json::json!({
            "name": "Class Proficiencies (Warlock)",
            "prerequisites": "CLASS.`Warlock`.LEVEL == LEVEL",
            "assign": [
                {
                    "expr": "HIT_DICE.SIDES = 8; WIS.SAVE.PROF = 1; CHA.SAVE.PROF = 1; PROF.LIGHT_ARMOR = 1; PROF.SIMPLE_WEAPONS = 1",
                    "when": "OnFeatureAdd"
                },
            ],
        }),
        serde_json::json!({
            "name": "Multiclass Proficiencies (Warlock)",
            "prerequisites": "CLASS.`Warlock`.LEVEL < LEVEL",
            "assign": [
                {
                    "expr": "HIT_DICE.SIDES = 8; PROF.LIGHT_ARMOR = 1",
                    "when": "OnFeatureAdd"
                },
            ],
        }),
    ] {
        let (name, def) = feature(value);
        features.insert(name, def);
    }

    let warlock: ClassDefinition = serde_json::from_value(serde_json::json!({
        "name": "Warlock",
        "levels": {
            "1": {
                "features": [
                    "Class Proficiencies (Warlock)",
                    "Multiclass Proficiencies (Warlock)",
                ],
            },
        },
    }))
    .expect("class def");
    let mut classes = BTreeMap::new();
    classes.insert(Box::from("Warlock"), warlock);

    let registry = RulesRegistry::for_test(
        FeaturesIndex(features),
        classes,
        BTreeMap::new(),
        BTreeMap::new(),
    );
    registry.await_ready().await;

    // Fighter 1 already applied; Warlock 1 just added to identity.
    let mut core = CharacterCore::default();
    core.identity.classes = vec![
        ClassLevel {
            class: "Fighter".into(),
            level: 1,
            hit_die_sides: 10,
            ..ClassLevel::default()
        },
        ClassLevel {
            class: "Warlock".into(),
            level: 1,
            ..ClassLevel::default()
        },
    ];
    core.applied.mark_level("Fighter", 1);

    let pending = registry.with_features_index_untracked(|features_index| {
        collect_pending_features(&core, &registry, features_index)
    });
    let names: Vec<&str> = pending.iter().map(|entry| &*entry.name).collect();
    assert!(
        names.contains(&"Multiclass Proficiencies (Warlock)"),
        "got {names:?}"
    );
    assert!(
        !names.contains(&"Class Proficiencies (Warlock)"),
        "got {names:?}"
    );

    registry.with_definitions(|caches| {
        registry.with_features_index_untracked(|features_index| {
            cascade(
                &mut core,
                &pending,
                features_index,
                caches,
                &|_| Vec::new(),
                &|_| None,
                false,
            );
        })
    });

    assert!(
        !core.saving_throws.contains(&Ability::Wisdom)
            && !core.saving_throws.contains(&Ability::Charisma),
        "multiclass must not add the new class's saving throws"
    );
    assert!(
        core.proficiencies.contains(&Proficiency::LightArmor),
        "multiclass Warlock adds light armor proficiency"
    );
    assert_eq!(
        core.identity.classes[1].hit_die_sides, 8,
        "hit die must be set for the multiclass"
    );
}
