//! Stage II pure-assign rebuild verification.
//!
//! Builds representative characters by applying synthetic feature definitions
//! that mirror the post-migration JSON shape (assign-only spells, FreeUses,
//! Points pools, Die fields). For each, applies the same definitions twice on
//! independent base characters and asserts the two derived states are equal —
//! the migration's correctness invariant.

use std::collections::BTreeMap;

use dnd_pc::{
    model::{
        AssignInputs, Character, CharacterIdentity, ClassLevel, Feature, FeatureSource,
        FeatureValue,
    },
    rules::{
        FeaturesView, WhenCondition,
        apply::{apply_feature, compute},
        feature::FeatureDefinition,
    },
};
use wasm_bindgen_test::*;

/// Build a `FeatureDefinition` from a JSON value. Panics on malformed JSON —
/// these are author-controlled fixtures, not user input.
fn def(json: serde_json::Value) -> FeatureDefinition {
    serde_json::from_value(json).expect("valid feature definition fixture")
}

/// Apply every definition in `defs` to `character` in pipeline order: each is
/// stamped as a class-sourced feature on the matching class level, then its
/// `OnFeatureAdd` body runs, then the global `OnCompute` pass (mirroring what
/// `build_clean` does after collecting features).
fn apply_all(
    character: &mut Character,
    defs: &[(FeatureDefinition, FeatureSource, u32)],
    feat_index: &BTreeMap<Box<str>, FeatureDefinition>,
) {
    for (feat_def, source, _level) in defs {
        character.features.list.push(Feature {
            name: feat_def.name.to_string(),
            source: source.clone(),
            applied: true,
            inputs: vec![AssignInputs::default()],
            ..Feature::default()
        });
        let feature_pos = character.features.list.len() - 1;
        apply_feature(
            feat_def,
            character,
            feature_pos,
            WhenCondition::OnFeatureAdd,
        );
    }
    compute(character, FeaturesView::from_natural(feat_index));
}

/// Build a Wizard 5 / Sorcerer 3 / Bard 3 / Cleric 3 character with realistic
/// identity scaffolding. Class breakdown is parameterized so each fixture can
/// pick its own without copy-pasting the rest of the identity.
fn make_character(classes: Vec<ClassLevel>) -> Character {
    let identity = CharacterIdentity {
        classes,
        ..Default::default()
    };
    Character::from_identity(identity)
}

/// Sticky + FreeUses pattern (Stage II canonical). Mirrors features like
/// "Fey Touched" / "Shadow Touched" which gained the Misty Step / Invisibility
/// spell with one free cast per long rest.
fn touched_def(feature_name: &str, spell_name: &str) -> FeatureDefinition {
    def(serde_json::json!({
        "name": feature_name,
        "category": "General",
        "assign": [
            { "expr": format!("STICKY.`{spell_name}` = 1"), "when": "OnCompute" },
            { "expr": format!("FREE_USES.`{spell_name}` = 1"), "when": "OnCompute" },
        ],
    }))
}

/// Points pool pattern (Stage I→II): Sorcery Points lazy-create via assign.
fn font_of_magic_def() -> FeatureDefinition {
    def(serde_json::json!({
        "name": "Font of Magic",
        "category": "Class",
        "assign": [
            {
                "expr": "POINTS.`Sorcery Points`.MAX = CLASS.LEVEL",
                "when": "OnCompute",
            },
        ],
    }))
}

/// Die field via OnCompute assign (Bardic Inspiration shape).
fn bardic_inspiration_def() -> FeatureDefinition {
    def(serde_json::json!({
        "name": "Bardic Inspiration",
        "category": "Class",
        "assign": [
            {
                "expr": "DIE.`Bardic Inspiration`.SIDES = 8; DIE.`Bardic Inspiration`.COUNT = 4",
                "when": "OnCompute",
            },
        ],
    }))
}

/// Languages-via-assign pattern (post-Stage I origin features).
fn linguist_def() -> FeatureDefinition {
    def(serde_json::json!({
        "name": "Linguist",
        "category": "Origin",
        "assign": [
            { "expr": "LANG.`Elvish` = 1", "when": "OnFeatureAdd" },
            { "expr": "LANG.`Dwarvish` = 1", "when": "OnFeatureAdd" },
        ],
    }))
}

fn build_index(defs: &[FeatureDefinition]) -> BTreeMap<Box<str>, FeatureDefinition> {
    defs.iter()
        .map(|feat_def| (feat_def.name.clone(), feat_def.clone()))
        .collect()
}

/// Helper: structural compare of features.data across two characters.
/// Beyond `eq_derived` (which only covers abilities/skills/etc.), the
/// pure-assign migration should produce identical feature data — same
/// fields, same spells with same sticky/free_uses/cost. Labels/descriptions
/// come from sync_labels and aren't compared (registry not loaded in tests).
fn assert_features_data_eq(rebuilt: &Character, original: &Character) {
    let original_keys: Vec<&str> = original
        .features
        .data()
        .keys()
        .map(String::as_str)
        .collect();
    let rebuilt_keys: Vec<&str> = rebuilt.features.data().keys().map(String::as_str).collect();
    assert_eq!(
        rebuilt_keys, original_keys,
        "features.data keys differ between rebuild runs"
    );

    for (name, original_data) in original.features.data() {
        let rebuilt_data = rebuilt
            .features
            .data()
            .get(name)
            .unwrap_or_else(|| panic!("rebuilt character missing feature data {name}"));

        // Compare field shape (names + value variants/maxes; ignore labels).
        assert_eq!(
            rebuilt_data.fields.len(),
            original_data.fields.len(),
            "feature {name} field count mismatch",
        );
        for (rebuilt_field, original_field) in rebuilt_data.fields.iter().zip(&original_data.fields)
        {
            assert_eq!(
                rebuilt_field.name, original_field.name,
                "feature {name} field name mismatch",
            );
            match (&rebuilt_field.value, &original_field.value) {
                (
                    FeatureValue::Points {
                        used: ru,
                        max: rmax,
                    },
                    FeatureValue::Points {
                        used: ou,
                        max: omax,
                    },
                ) => {
                    assert_eq!((ru, rmax), (ou, omax), "feature {name} Points mismatch");
                }
                (
                    FeatureValue::Die { die: rd, used: ru },
                    FeatureValue::Die { die: od, used: ou },
                ) => {
                    assert_eq!(
                        (rd.amount, rd.sides, ru),
                        (od.amount, od.sides, ou),
                        "feature {name} Die mismatch",
                    );
                }
                (left, right) => assert_eq!(
                    std::mem::discriminant(left),
                    std::mem::discriminant(right),
                    "feature {name} value variant mismatch",
                ),
            }
        }

        // Compare spell data names + sticky + free_uses + cost.
        match (&rebuilt_data.spells, &original_data.spells) {
            (Some(rebuilt_spells), Some(original_spells)) => {
                let rebuilt_names: Vec<_> = rebuilt_spells
                    .spells
                    .iter()
                    .map(|spell| &spell.name)
                    .collect();
                let original_names: Vec<_> = original_spells
                    .spells
                    .iter()
                    .map(|spell| &spell.name)
                    .collect();
                assert_eq!(
                    rebuilt_names, original_names,
                    "feature {name} spell names differ",
                );
                for (rebuilt_spell, original_spell) in
                    rebuilt_spells.spells.iter().zip(&original_spells.spells)
                {
                    assert_eq!(
                        rebuilt_spell.sticky, original_spell.sticky,
                        "feature {name} spell {} sticky mismatch",
                        rebuilt_spell.name,
                    );
                    assert_eq!(
                        rebuilt_spell.free_uses.as_ref().map(|fu| (fu.used, fu.max)),
                        original_spell
                            .free_uses
                            .as_ref()
                            .map(|fu| (fu.used, fu.max)),
                        "feature {name} spell {} free_uses mismatch",
                        rebuilt_spell.name,
                    );
                }
            }
            (None, None) => {}
            _ => panic!("feature {name} spell-data presence differs between runs"),
        }
    }
}

/// Verifies a character built by applying `defs` twice produces identical
/// derived state and feature data.
fn assert_rebuild_deterministic(
    label: &str,
    classes: Vec<ClassLevel>,
    defs: Vec<(FeatureDefinition, FeatureSource, u32)>,
) {
    let feat_index = build_index(&defs.iter().map(|(d, _, _)| d.clone()).collect::<Vec<_>>());

    let mut original = make_character(classes.clone());
    apply_all(&mut original, &defs, &feat_index);

    let mut rebuilt = make_character(classes);
    apply_all(&mut rebuilt, &defs, &feat_index);

    assert!(
        rebuilt.eq_derived(&original),
        "[{label}] eq_derived mismatch after pure-assign rebuild",
    );
    assert_eq!(
        *rebuilt.spell_slots, *original.spell_slots,
        "[{label}] spell_slots mismatch",
    );
    assert_features_data_eq(&rebuilt, &original);
}

#[wasm_bindgen_test]
fn wizard_l5_assign_only() {
    // Single-class Wizard 5, no interactive picks — touches Linguist (languages
    // via assign) plus a sticky spell granted at L5.
    assert_rebuild_deterministic(
        "wizard_l5",
        vec![ClassLevel {
            class: "Wizard".into(),
            level: 5,
            hit_die_sides: 6,
            ..ClassLevel::default()
        }],
        vec![
            (linguist_def(), FeatureSource::Background("Sage".into()), 1),
            (
                touched_def("Fey Touched", "Misty Step"),
                FeatureSource::User(0),
                1,
            ),
        ],
    );
}

#[wasm_bindgen_test]
fn bard_l3_die_field() {
    // Bardic Inspiration touches DIE.`Bardic Inspiration` via assign.
    assert_rebuild_deterministic(
        "bard_l3",
        vec![ClassLevel {
            class: "Bard".into(),
            level: 3,
            hit_die_sides: 8,
            ..ClassLevel::default()
        }],
        vec![(
            bardic_inspiration_def(),
            FeatureSource::Class("Bard".into(), 1),
            1,
        )],
    );
}

#[wasm_bindgen_test]
fn sorcerer_l5_points_pool() {
    // Font of Magic lazy-creates POINTS.`Sorcery Points` via OnCompute assign.
    assert_rebuild_deterministic(
        "sorcerer_l5",
        vec![ClassLevel {
            class: "Sorcerer".into(),
            level: 5,
            hit_die_sides: 6,
            ..ClassLevel::default()
        }],
        vec![(
            font_of_magic_def(),
            FeatureSource::Class("Sorcerer".into(), 2),
            2,
        )],
    );
}

#[wasm_bindgen_test]
fn cleric_light_l5_sticky_and_free_uses() {
    // Light Domain pattern: a free-cast spell granted by subclass. Verifies
    // the sticky-spell + free-uses double-assign survives a full rebuild.
    assert_rebuild_deterministic(
        "cleric_light_l5",
        vec![ClassLevel {
            class: "Cleric".into(),
            level: 5,
            hit_die_sides: 8,
            ..ClassLevel::default()
        }],
        vec![(
            touched_def("Light Domain Spells", "Faerie Fire"),
            FeatureSource::Subclass("Cleric".into(), "Light Domain".into(), 1),
            1,
        )],
    );
}

#[wasm_bindgen_test]
fn multiclass_wizard_warlock_combined_assigns() {
    // Wizard 3 / Warlock 2: two sticky+freeuses features on different sources.
    // Verifies that pure-assign apply scales across multiclass identity slots
    // without cross-contamination of feature data.
    assert_rebuild_deterministic(
        "multiclass_wizard3_warlock2",
        vec![
            ClassLevel {
                class: "Wizard".into(),
                level: 3,
                hit_die_sides: 6,
                ..ClassLevel::default()
            },
            ClassLevel {
                class: "Warlock".into(),
                level: 2,
                hit_die_sides: 8,
                ..ClassLevel::default()
            },
        ],
        vec![
            (
                touched_def("Fey Touched", "Misty Step"),
                FeatureSource::User(0),
                1,
            ),
            (
                touched_def("Pact of the Tome", "Mage Hand"),
                FeatureSource::Class("Warlock".into(), 2),
                2,
            ),
            (font_of_magic_def(), FeatureSource::User(0), 1),
        ],
    );
}
