//! Tier-A smoke tests for the apply cascade: synthetic registries, no
//! Leptos scope, no UI. Exercises cascade / apply_pending / build_clean
//! against scenarios from `docs/superpowers/plans/manual-smoke-test.md`.
//!
//! Coverage:
//! - #2 / #15 Level-up + ASI replacement at L8 with feat replacement.
//! - #10 Multiclass prereq (class swap, prereq filter).
//! - #13 Replacement (placeholder → real feat).
//! - #16 Multi-pick stability cascade-side (chain whitelist contract).

use std::collections::{BTreeMap, BTreeSet};

use dnd_pc::{
    model::{
        Applied, AssignInputs, Character, CharacterCore, ClassLevel, Feature, FeatureCategory,
        FeatureSource, IdentitySlot,
    },
    rules::{
        ApplyInputs, BackgroundDefinition, ClassDefinition, FeatureDefinition, FeaturesView,
        ReplaceWith, SpeciesDefinition,
        apply::{
            DefinitionCaches, FeatureKey, PendingFeature, cascade, make_inputs_for,
            make_replacement_for,
        },
        make_system_feature,
    },
};
use wasm_bindgen_test::*;

fn class_def(name: &str, levels: serde_json::Value) -> ClassDefinition {
    serde_json::from_value(serde_json::json!({
        "name": name,
        "hit_die": 8,
        "levels": levels,
    }))
    .expect("class def")
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

fn applied_feature(name: &str, source: FeatureSource) -> Feature {
    Feature {
        name: name.to_string(),
        source,
        applied: true,
        ..Feature::default()
    }
}

fn wizard_l5() -> Character {
    let mut character = Character::default();
    character.identity.classes = vec![ClassLevel {
        class: "Wizard".into(),
        level: 5,
        ..ClassLevel::default()
    }];
    character.applied = Applied::default();
    for level in 1..=5u32 {
        character.applied.mark_level("Wizard", level);
        character.features.list.push(applied_feature(
            "Wizard L_n",
            FeatureSource::Class("Wizard".into(), level),
        ));
    }
    character
}

fn empty_caches<'a>(
    classes: &'a BTreeMap<Box<str>, ClassDefinition>,
    species: &'a BTreeMap<Box<str>, SpeciesDefinition>,
    bgs: &'a BTreeMap<Box<str>, BackgroundDefinition>,
) -> DefinitionCaches<'a> {
    DefinitionCaches {
        classes,
        species,
        backgrounds: bgs,
    }
}

/// #16 cascade-side: speculative cascade with one section's whitelist on
/// `inputs_for` does not bleed into chain[predecessor]'s subscriptions —
/// chain Effect-publisher uses an `inputs_for` filtered to its own key and
/// expects cascade follow-ups to push as `applied=false` placeholders, so
/// `Features::contains(applied=true)` returns false on second pass.
#[wasm_bindgen_test]
fn speculative_cascade_pushes_followups_as_unapplied() {
    let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
    feat_index.insert(
        "Wizard".into(),
        make_system_feature("Wizard".into(), IdentitySlot::Class),
    );
    feat_index.insert(
        "Class Proficiencies (Wizard)".into(),
        plain_feat("Class Proficiencies (Wizard)"),
    );
    let view = FeaturesView::from_natural(&feat_index);
    let class_index: BTreeMap<Box<str>, ClassDefinition> = [(
        "Wizard".into(),
        class_def(
            "Wizard",
            serde_json::json!({ "1": { "features": ["Class Proficiencies (Wizard)"] } }),
        ),
    )]
    .into_iter()
    .collect();
    let species: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
    let bgs: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
    let caches = empty_caches(&class_index, &species, &bgs);

    let mut snapshot = CharacterCore::default();
    let pending = PendingFeature {
        name: "Wizard".into(),
        source: FeatureSource::User(1),
        level: 1,
        replaces: None,
    };
    let inputs_for = |_: &FeatureKey| Vec::<AssignInputs>::new();
    let replacement_for = |_: &FeatureKey| -> Option<String> { None };
    cascade(
        &mut snapshot,
        std::slice::from_ref(&pending),
        view,
        caches,
        &inputs_for,
        &replacement_for,
        true, // speculative
    );

    let prof = snapshot
        .features
        .list
        .iter()
        .find(|feature| feature.name == "Class Proficiencies (Wizard)")
        .expect("speculative cascade emits Wizard L1 follow-up");
    assert!(
        !prof.applied,
        "speculative cascade must push follow-ups as applied=false, got {:?}",
        prof
    );
    assert!(
        snapshot
            .applied
            .levels
            .get("Wizard")
            .is_none_or(|levels| levels.is_empty()),
        "speculative cascade must skip apply_identity_flags so applied.levels stays empty: {:?}",
        snapshot.applied.levels
    );
}

/// #10 multiclass: speculative cascade with replacement Class Level →
/// Fighter on a Wizard L5 grows identity to include Fighter L1 and
/// surfaces Fighter L1 features as unapplied for the modal.
#[wasm_bindgen_test]
fn class_level_replacement_to_new_class_emits_l1_followups() {
    let original = wizard_l5();

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
    let view = FeaturesView::from_natural(&feat_index);

    let class_index: BTreeMap<Box<str>, ClassDefinition> = [
        (
            "Wizard".into(),
            class_def(
                "Wizard",
                serde_json::json!({ "1": { "features": ["Wizard L_n"] } }),
            ),
        ),
        (
            "Fighter".into(),
            class_def(
                "Fighter",
                serde_json::json!({
                    "1": { "features": ["Class Proficiencies (Fighter)", "Fighting Style"] }
                }),
            ),
        ),
    ]
    .into_iter()
    .collect();
    let species: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
    let bgs: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
    let caches = empty_caches(&class_index, &species, &bgs);

    let mut snapshot = original.core.clone();
    let placeholder = PendingFeature {
        name: "Class Level".into(),
        source: FeatureSource::User(6),
        level: 6,
        replaces: None,
    };
    let inputs_for = |_: &FeatureKey| Vec::<AssignInputs>::new();
    let replacement_for = |key: &FeatureKey| -> Option<String> {
        (key.name == "Class Level").then(|| "Fighter".to_string())
    };
    cascade(
        &mut snapshot,
        std::slice::from_ref(&placeholder),
        view,
        caches,
        &inputs_for,
        &replacement_for,
        true, // speculative
    );

    assert!(
        snapshot
            .identity
            .classes
            .iter()
            .any(|class_level| class_level.class == "Fighter" && class_level.level == 1),
        "Fighter L1 must enter identity after speculative cascade: {:?}",
        snapshot.identity.classes
    );
    let l1_follow_up = snapshot
        .features
        .list
        .iter()
        .find(|feature| feature.name == "Class Proficiencies (Fighter)")
        .expect("Fighter L1 follow-up surfaced");
    assert!(
        !l1_follow_up.applied,
        "speculative follow-up must remain applied=false: {:?}",
        l1_follow_up
    );
}

/// #5 silent-commit identity gate: `eq_derived` matches when only
/// non-derived state differs (e.g. `core.notes`). Solver uses this on a
/// rebuild candidate to decide whether to skip the modal.
#[wasm_bindgen_test]
fn eq_derived_ignores_non_apply_state() {
    let mut a = Character::default();
    a.identity.classes = vec![ClassLevel {
        class: "Wizard".into(),
        level: 1,
        ..ClassLevel::default()
    }];
    let mut b = a.clone();
    b.notes.push(dnd_pc::model::Note {
        text: "scratch".into(),
        ..Default::default()
    });
    assert!(a.core.eq_derived(&b.core));
}

/// #8 long rest restores HP to max, resets death saves, refills slots.
#[wasm_bindgen_test]
fn long_rest_restores_resources() {
    let mut character = Character::default();
    character.combat.hp_max = 30;
    character.combat.hp_current = 5;
    character.combat.death_save_failures = 2;
    character.combat.death_save_successes = 1;
    character.identity.classes = vec![ClassLevel {
        class: "Wizard".into(),
        level: 5,
        hit_die_sides: 6,
        hit_dice_used: 3,
        ..ClassLevel::default()
    }];

    character.long_rest();

    assert_eq!(character.combat.hp_current, 30);
    assert_eq!(character.combat.death_save_failures, 0);
    assert_eq!(character.combat.death_save_successes, 0);
    assert_eq!(
        character.identity.classes[0].hit_dice_used, 0,
        "long rest resets hit dice used"
    );
}

/// #15 ASI replacement at L8 — placeholder ASI source replaced with a feat
/// (Tough). Asserts ASI L4 stays as-is, Tough lands at Class(Wizard, 8).
#[wasm_bindgen_test]
fn asi_replacement_keeps_prior_asi_intact() {
    let mut character = Character::default();
    character.identity.classes = vec![ClassLevel {
        class: "Wizard".into(),
        level: 8,
        ..ClassLevel::default()
    }];
    character.features.list.push(applied_feature(
        "Ability Score Improvement",
        FeatureSource::Class("Wizard".into(), 4),
    ));

    let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
    feat_index.insert(
        "Ability Score Improvement".into(),
        FeatureDefinition {
            name: "Ability Score Improvement".into(),
            stackable: true,
            category: FeatureCategory::General,
            replace_with: ReplaceWith::Any,
            spells: None,
            actions: BTreeMap::new(),
            assign: None,
            prerequisites: None,
        },
    );
    feat_index.insert("Tough".into(), plain_feat("Tough"));
    let view = FeaturesView::from_natural(&feat_index);
    let class_index: BTreeMap<Box<str>, ClassDefinition> = BTreeMap::new();
    let species: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
    let bgs: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
    let caches = empty_caches(&class_index, &species, &bgs);

    let l8_asi = PendingFeature {
        name: "Ability Score Improvement".into(),
        source: FeatureSource::Class("Wizard".into(), 8),
        level: 8,
        replaces: None,
    };
    let inputs_for = |_: &FeatureKey| Vec::<AssignInputs>::new();
    let replacement_for = |key: &FeatureKey| -> Option<String> {
        (key.name == "Ability Score Improvement").then(|| "Tough".to_string())
    };
    cascade(
        &mut character.core,
        std::slice::from_ref(&l8_asi),
        view,
        caches,
        &inputs_for,
        &replacement_for,
        false, // real apply
    );

    let asi_l4 = character
        .core
        .features
        .list
        .iter()
        .find(|feature| {
            feature.name == "Ability Score Improvement"
                && matches!(&feature.source, FeatureSource::Class(name, 4) if name.as_ref() == "Wizard")
        })
        .expect("L4 ASI preserved");
    assert!(asi_l4.applied);

    let tough_l8 = character
        .core
        .features
        .list
        .iter()
        .find(|feature| {
            feature.name == "Tough"
                && matches!(&feature.source, FeatureSource::Class(name, 8) if name.as_ref() == "Wizard")
        })
        .expect("Tough lands at Wizard L8 source");
    assert!(tough_l8.applied);

    let stray_l8_asi = character.core.features.list.iter().find(|feature| {
        feature.name == "Ability Score Improvement"
            && matches!(&feature.source, FeatureSource::Class(name, 8) if name.as_ref() == "Wizard")
    });
    assert!(
        stray_l8_asi.is_none(),
        "L8 ASI placeholder must be replaced, not retained: {:?}",
        stray_l8_asi
    );
}

/// User-reported regression: rebuild lost the prior swap. Original character
/// at L8 has vanilla ASI on L4 and Spell-Sniper-replaces-ASI on L8. Without
/// the user reopening any modal, rebuild's `make_replacement_for` must
/// recover the L8 swap from `original.features.replaces` while leaving the
/// L4 vanilla ASI alone — source-aware lookup, not name-only.
#[wasm_bindgen_test]
fn rebuild_recovers_per_source_replacement_from_original() {
    let mut original = Character::default();
    original.identity.classes = vec![ClassLevel {
        class: "Wizard".into(),
        level: 8,
        ..ClassLevel::default()
    }];
    // L4: vanilla ASI (no `replaces`).
    original.features.list.push(Feature {
        name: "Ability Score Improvement".into(),
        source: FeatureSource::Class("Wizard".into(), 4),
        category: FeatureCategory::General,
        applied: true,
        inputs: vec![AssignInputs {
            args: vec![0, 1, 0, 0, 0, 1],
            ..AssignInputs::default()
        }],
        ..Feature::default()
    });
    // L8: Spell Sniper recorded as a swap of ASI.
    original.features.list.push(Feature {
        name: "Spell Sniper".into(),
        source: FeatureSource::Class("Wizard".into(), 8),
        category: FeatureCategory::General,
        applied: true,
        inputs: vec![AssignInputs {
            args: vec![0, 0, 1],
            ..AssignInputs::default()
        }],
        replaces: Some("Ability Score Improvement".into()),
        ..Feature::default()
    });

    let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
    feat_index.insert(
        "Ability Score Improvement".into(),
        FeatureDefinition {
            name: "Ability Score Improvement".into(),
            stackable: true,
            category: FeatureCategory::General,
            replace_with: ReplaceWith::Any,
            spells: None,
            actions: BTreeMap::new(),
            assign: None,
            prerequisites: None,
        },
    );
    feat_index.insert("Spell Sniper".into(), plain_feat("Spell Sniper"));
    let view = FeaturesView::from_natural(&feat_index);
    let class_index: BTreeMap<Box<str>, ClassDefinition> = BTreeMap::new();
    let species: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
    let bgs: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
    let caches = empty_caches(&class_index, &species, &bgs);

    // Rebuild starts with no modal-supplied inputs — the cascade must
    // recover the swap purely from `original.features`.
    let extra = ApplyInputs::default();
    let pending_keys = BTreeSet::new();
    let inputs_for = make_inputs_for(view, &original, &extra);
    let replacement_for = make_replacement_for(view, &original, &extra, &pending_keys);

    let mut clean = CharacterCore::default();
    // Apply L4 ASI follow-up first (vanilla — source-aware lookup must
    // return None here).
    let l4_pending = PendingFeature {
        name: "Ability Score Improvement".into(),
        source: FeatureSource::Class("Wizard".into(), 4),
        level: 4,
        replaces: None,
    };
    cascade(
        &mut clean,
        std::slice::from_ref(&l4_pending),
        view,
        caches,
        &inputs_for,
        &replacement_for,
        false,
    );
    // Apply L8 ASI follow-up — source-aware lookup must redirect to
    // Spell Sniper.
    let l8_pending = PendingFeature {
        name: "Ability Score Improvement".into(),
        source: FeatureSource::Class("Wizard".into(), 8),
        level: 8,
        replaces: None,
    };
    cascade(
        &mut clean,
        std::slice::from_ref(&l8_pending),
        view,
        caches,
        &inputs_for,
        &replacement_for,
        false,
    );

    // L4 stays vanilla ASI with the prior abilities pick.
    let l4 = clean
        .features
        .list
        .iter()
        .find(|feature| {
            feature.name == "Ability Score Improvement"
                && matches!(&feature.source, FeatureSource::Class(name, 4) if name.as_ref() == "Wizard")
        })
        .expect("L4 ASI preserved");
    assert!(l4.applied, "L4 ASI must land applied");
    assert_eq!(
        l4.inputs[0].args,
        vec![0, 1, 0, 0, 0, 1],
        "L4 ASI inputs come from original.features"
    );
    assert_eq!(l4.replaces, None, "L4 was vanilla — no replaces tag");

    // L8 row resolves to Spell Sniper, with the original Spell Sniper inputs
    // and the placeholder name recorded in `replaces`.
    let l8 = clean
        .features
        .list
        .iter()
        .find(|feature| {
            feature.name == "Spell Sniper"
                && matches!(&feature.source, FeatureSource::Class(name, 8) if name.as_ref() == "Wizard")
        })
        .expect("Spell Sniper recovered at Wizard L8 source");
    assert!(l8.applied, "Spell Sniper must land applied");
    assert_eq!(
        l8.inputs[0].args,
        vec![0, 0, 1],
        "Spell Sniper inputs come from original.features"
    );
    assert_eq!(
        l8.replaces.as_deref(),
        Some("Ability Score Improvement"),
        "swap row carries the placeholder it replaces"
    );

    // No leftover vanilla ASI at L8.
    assert!(
        !clean.features.list.iter().any(|feature| {
            feature.name == "Ability Score Improvement"
                && matches!(&feature.source, FeatureSource::Class(name, 8) if name.as_ref() == "Wizard")
        }),
        "L8 ASI placeholder must not survive"
    );
}

/// Rebuild modal: user unchecks L4 Spell-Sniper-replaces-ASI swap and
/// moves it to L8. Explicit `replacement = None` at L4 must not fall back
/// to `original.features`.
#[wasm_bindgen_test]
fn rebuild_uncheck_replace_at_one_source_and_check_at_another() {
    use dnd_pc::rules::ApplyInput;

    let mut original = Character::default();
    original.identity.classes = vec![ClassLevel {
        class: "Wizard".into(),
        level: 8,
        ..ClassLevel::default()
    }];
    original.features.list.push(Feature {
        name: "Spell Sniper".into(),
        source: FeatureSource::Class("Wizard".into(), 4),
        category: FeatureCategory::General,
        applied: true,
        inputs: vec![AssignInputs {
            args: vec![0, 0, 1],
            ..AssignInputs::default()
        }],
        replaces: Some("Ability Score Improvement".into()),
        ..Feature::default()
    });

    let mut feat_index: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
    feat_index.insert(
        "Ability Score Improvement".into(),
        FeatureDefinition {
            name: "Ability Score Improvement".into(),
            stackable: true,
            category: FeatureCategory::General,
            replace_with: ReplaceWith::Any,
            spells: None,
            actions: BTreeMap::new(),
            assign: None,
            prerequisites: None,
        },
    );
    feat_index.insert("Spell Sniper".into(), plain_feat("Spell Sniper"));
    let view = FeaturesView::from_natural(&feat_index);
    let class_index: BTreeMap<Box<str>, ClassDefinition> = BTreeMap::new();
    let species: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
    let bgs: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
    let caches = empty_caches(&class_index, &species, &bgs);

    let l4_key = FeatureKey::new(
        "Ability Score Improvement",
        FeatureSource::Class("Wizard".into(), 4),
    );
    let l8_placeholder_key = FeatureKey::new(
        "Ability Score Improvement",
        FeatureSource::Class("Wizard".into(), 8),
    );
    let l8_swap_key = FeatureKey::new(
        "Spell Sniper",
        FeatureSource::Class("Wizard".into(), 8),
    );

    let mut extra = ApplyInputs::default();
    extra.insert(
        l4_key,
        ApplyInput {
            inputs: vec![AssignInputs {
                args: vec![0, 1, 0, 0, 0, 1],
                ..AssignInputs::default()
            }],
            replacement: None,
        },
    );
    extra.insert(
        l8_placeholder_key,
        ApplyInput {
            inputs: vec![],
            replacement: Some("Spell Sniper".into()),
        },
    );
    extra.insert(
        l8_swap_key,
        ApplyInput {
            inputs: vec![AssignInputs {
                args: vec![0, 0, 1],
                ..AssignInputs::default()
            }],
            replacement: None,
        },
    );

    let pending_keys = BTreeSet::new();
    let inputs_for = make_inputs_for(view, &original, &extra);
    let replacement_for = make_replacement_for(view, &original, &extra, &pending_keys);

    let mut clean = CharacterCore::default();
    let l4_pending = PendingFeature {
        name: "Ability Score Improvement".into(),
        source: FeatureSource::Class("Wizard".into(), 4),
        level: 4,
        replaces: None,
    };
    cascade(
        &mut clean,
        std::slice::from_ref(&l4_pending),
        view,
        caches,
        &inputs_for,
        &replacement_for,
        false,
    );
    let l8_pending = PendingFeature {
        name: "Ability Score Improvement".into(),
        source: FeatureSource::Class("Wizard".into(), 8),
        level: 8,
        replaces: None,
    };
    cascade(
        &mut clean,
        std::slice::from_ref(&l8_pending),
        view,
        caches,
        &inputs_for,
        &replacement_for,
        false,
    );

    let l4 = clean
        .features
        .list
        .iter()
        .find(|feature| {
            feature.name == "Ability Score Improvement"
                && matches!(&feature.source, FeatureSource::Class(name, 4) if name.as_ref() == "Wizard")
        })
        .expect("L4 must land as vanilla ASI");
    assert!(l4.applied);
    assert_eq!(l4.replaces, None);

    assert!(
        !clean.features.list.iter().any(|feature| {
            feature.name == "Spell Sniper"
                && matches!(&feature.source, FeatureSource::Class(name, 4) if name.as_ref() == "Wizard")
        }),
        "Spell Sniper must not be applied at L4"
    );

    let l8 = clean
        .features
        .list
        .iter()
        .find(|feature| {
            feature.name == "Spell Sniper"
                && matches!(&feature.source, FeatureSource::Class(name, 8) if name.as_ref() == "Wizard")
        })
        .expect("L8 must land as Spell Sniper");
    assert!(l8.applied);
    assert_eq!(l8.replaces.as_deref(), Some("Ability Score Improvement"));
}
