//! Reproducers for rebuild-modal bugs surfaced when the user pencil-edits
//! a background row (e.g. Entertainer → Criminal) and clicks Rebuild:
//!
//! 1. `rebuild_after_background_swap_collects_new_background_features` —
//!    `prepare_rebuild` collects pending features off the stale
//!    `identity.background`, so the modal asks for the OLD background's
//!    abilities instead of the NEW one the user just picked.
//!
//! 2. `rebuild_does_not_silently_commit_when_pending_feat_has_empty_inputs` —
//!    when a previous rebuild left an interactive feat `applied=false` with
//!    empty inputs (because of the bug above), the next Rebuild click must
//!    surface the modal; currently it silently commits with solver- fallback
//!    zeros and bypasses the user.

#![cfg(target_arch = "wasm32")]

use std::collections::BTreeMap;

use dnd_pc::{
    model::{
        Applied, AssignInputs, Character, Expr, Feature, FeatureCategory, FeatureSource,
        IdentitySlot, Skill,
    },
    rules::{
        BackgroundDefinition, FeatureDefinition, FeaturesIndex, ReplaceWith, RulesRegistry,
        WhenCondition,
        apply::{
            apply_assignments_with_inputs, build_clean, level_up_plan, prepare_rebuild,
            synthesize_apply_inputs,
        },
        feature::Assignment,
    },
    vecset::VecSet,
};
use leptos::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

fn placeholder_def(name: &str, slot: IdentitySlot) -> FeatureDefinition {
    FeatureDefinition {
        name: name.into(),
        stackable: false,
        category: FeatureCategory::Generation,
        replace_with: ReplaceWith::Category(FeatureCategory::System(slot)),
        spells: None,
        actions: BTreeMap::new(),
        assign: None,
        prerequisites: None,
    }
}

fn onadd_assign(expr_str: &str) -> Vec<Assignment> {
    let expr: Expr = expr_str.parse().expect("expression parses");
    vec![Assignment {
        expr,
        when: WhenCondition::OnFeatureAdd,
    }]
}

fn background_marker_def(name: &str) -> FeatureDefinition {
    // Mirrors `make_system_feature` (registry.rs): synthesized System(Background)
    // markers carry `BACKGROUND.\`<name>\` = 1` so applying the row fires
    // IdentityChange::Background and unblocks derive_identity_follow_ups.
    let expr_str = format!("BACKGROUND.`{name}` = 1");
    FeatureDefinition {
        name: name.into(),
        stackable: false,
        category: FeatureCategory::System(IdentitySlot::Background),
        replace_with: ReplaceWith::None,
        spells: None,
        actions: BTreeMap::new(),
        assign: Some(onadd_assign(&expr_str)),
        prerequisites: None,
    }
}

fn background_ability_def(name: &str, abilities: &str) -> FeatureDefinition {
    // Real Entertainer/Criminal Background Abilities expression — uses
    // `in(@ARG, 0, 2)` so `scan_arg_range` produces a valid range and the
    // assign survives `prepare_rebuild`'s emit filter.
    let expr_str = format!(
        "with(@ABILITY({abilities}), guard(fold(and, @, in(@ARG, 0, 2) and @ + @ARG <= 20) and fold(+, @, @ARG) == 3, each(@, if(@ < 20, @ += @ARG))))",
    );
    FeatureDefinition {
        name: name.into(),
        stackable: false,
        category: FeatureCategory::Origin,
        replace_with: ReplaceWith::None,
        spells: None,
        actions: BTreeMap::new(),
        assign: Some(onadd_assign(&expr_str)),
        prerequisites: None,
    }
}

fn plain_feat_def(name: &str) -> FeatureDefinition {
    FeatureDefinition {
        name: name.into(),
        stackable: false,
        category: FeatureCategory::Origin,
        replace_with: ReplaceWith::None,
        spells: None,
        actions: BTreeMap::new(),
        assign: None,
        prerequisites: None,
    }
}

fn build_registry() -> RulesRegistry {
    let mut features = BTreeMap::<Box<str>, FeatureDefinition>::new();
    features.insert(
        "Generation: Background".into(),
        placeholder_def("Generation: Background", IdentitySlot::Background),
    );
    features.insert("Entertainer".into(), background_marker_def("Entertainer"));
    features.insert("Criminal".into(), background_marker_def("Criminal"));
    features.insert(
        "Background Abilities (Entertainer)".into(),
        background_ability_def("Background Abilities (Entertainer)", "STR, DEX, CHA"),
    );
    features.insert(
        "Background Abilities (Criminal)".into(),
        background_ability_def("Background Abilities (Criminal)", "DEX, CON, INT"),
    );
    features.insert("Musician".into(), plain_feat_def("Musician"));
    features.insert("Alert".into(), plain_feat_def("Alert"));

    let mut backgrounds = BTreeMap::<Box<str>, BackgroundDefinition>::new();
    backgrounds.insert(
        "Entertainer".into(),
        BackgroundDefinition {
            name: "Entertainer".into(),
            features: VecSet::from(vec![
                "Background Abilities (Entertainer)".to_string(),
                "Musician".to_string(),
            ]),
        },
    );
    backgrounds.insert(
        "Criminal".into(),
        BackgroundDefinition {
            name: "Criminal".into(),
            features: VecSet::from(vec![
                "Background Abilities (Criminal)".to_string(),
                "Alert".to_string(),
            ]),
        },
    );

    RulesRegistry::for_test(
        FeaturesIndex(features),
        BTreeMap::new(),
        BTreeMap::new(),
        backgrounds,
    )
}

/// Mirror the saved file `Элина __ (Copy).dnd (1).json`: an Entertainer-
/// applied character where the user pencil-edited the System(Background) row
/// to swap Entertainer → Criminal. The Criminal row is `applied=false`
/// (its identity-changing assign hasn't fired yet), but `identity.background`
/// still says "Entertainer" because edit-modal didn't touch identity.
fn character_after_edit_swap() -> Character {
    // Minimal repro: only background populated. Species/classes are
    // intentionally empty so the test registry doesn't need to wire up
    // SpeciesDefinition / ClassDefinition; the background-swap bug
    // surfaces purely from the background path.
    let mut character = Character::default();
    character.identity.background = "Entertainer".into();
    character.applied = Applied {
        species: false,
        background: true,
        levels: BTreeMap::new(),
    };

    // The row the user edited: was "Entertainer" with replaces=Generation:
    // Background; pencil swap renamed it to "Criminal". applied=false marks
    // the assign as not yet fired.
    character.features.list.push(Feature {
        name: "Criminal".into(),
        source: FeatureSource::User(0),
        applied: false,
        category: FeatureCategory::System(IdentitySlot::Background),
        replaces: Some("Generation: Background".into()),
        ..Feature::default()
    });
    // The old background's features, still applied from the original
    // Entertainer build.
    character.features.list.push(Feature {
        name: "Background Abilities (Entertainer)".into(),
        source: FeatureSource::Background("Entertainer".into()),
        applied: true,
        category: FeatureCategory::Origin,
        inputs: vec![AssignInputs {
            args: vec![0, 0, 0],
            ..AssignInputs::default()
        }],
        ..Feature::default()
    });
    character.features.list.push(Feature {
        name: "Musician".into(),
        source: FeatureSource::Background("Entertainer".into()),
        applied: true,
        category: FeatureCategory::Origin,
        ..Feature::default()
    });
    character
}

/// Clicking Rebuild after a pencil-edit that swapped the background row
/// from Entertainer to Criminal MUST show the new background's pending
/// features (Background Abilities (Criminal), Alert) — the user's intent
/// is "I am now Criminal." `prepare_rebuild` currently collects pending
/// off `identity.background`, which is stale until the swap row's assigns
/// fire, so the modal shows Background Abilities (Entertainer) instead.
#[wasm_bindgen_test]
async fn rebuild_after_background_swap_collects_new_background_features() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.set();

    let registry = build_registry();
    registry.await_ready().await;

    let character = character_after_edit_swap();
    let preview = prepare_rebuild(character, &registry).expect("prepare_rebuild");

    let pending_names: Vec<String> = preview
        .pending
        .iter()
        .map(|input| input.feature_name.to_string())
        .collect();

    assert!(
        pending_names
            .iter()
            .any(|name| name == "Background Abilities (Criminal)"),
        "rebuild after Entertainer→Criminal swap must surface Criminal background features; got {pending_names:?}",
    );
    assert!(
        !pending_names
            .iter()
            .any(|name| name == "Background Abilities (Entertainer)"),
        "rebuild must NOT ask user for stale Entertainer features after swap; got {pending_names:?}",
    );
}

/// Mirror the saved file `Элина __ (Copy).dnd (2).json`: the previous rebuild
/// committed (background applied, identity migrated to Criminal), but
/// `Background Abilities (Criminal)` landed `applied=false` with empty
/// inputs because the modal had collected user input under the wrong key
/// (the stale-identity bug above). A second Rebuild click must NOT silently
/// re-apply with solver-fallback zeros — it must surface the modal for user
/// input.
fn character_after_stage2_partial_rebuild() -> Character {
    let mut character = Character::default();
    // Minimal repro: only background populated. Species/class drop avoids
    // having to wire SpeciesDefinition/ClassDefinition into the test
    // registry; the silent-commit bug surfaces purely from the background
    // path.
    character.identity.background = "Criminal".into();
    character.applied = Applied {
        species: false,
        background: true,
        levels: BTreeMap::new(),
    };

    // Criminal background row, fully applied after the first rebuild.
    character.features.list.push(Feature {
        name: "Criminal".into(),
        source: FeatureSource::User(0),
        applied: true,
        category: FeatureCategory::System(IdentitySlot::Background),
        replaces: Some("Generation: Background".into()),
        ..Feature::default()
    });
    // Background Abilities (Criminal) — pushed by cascade as applied=false
    // because the modal's collected inputs were keyed under the OLD
    // Entertainer's name, leaving the Criminal slot with empty stored
    // inputs.
    character.features.list.push(Feature {
        name: "Background Abilities (Criminal)".into(),
        source: FeatureSource::Background("Criminal".into()),
        applied: false,
        category: FeatureCategory::Origin,
        inputs: Vec::new(),
        ..Feature::default()
    });
    // Alert applied normally (no interactive inputs).
    character.features.list.push(Feature {
        name: "Alert".into(),
        source: FeatureSource::Background("Criminal".into()),
        applied: true,
        category: FeatureCategory::Origin,
        ..Feature::default()
    });
    character
}

/// `Background Abilities (Criminal)` lives in `features.list` with
/// `applied=false` and empty `inputs`. Clicking Rebuild MUST open the args
/// modal — the user has never supplied ARGs for this row. The current
/// silent-commit path skips the modal: `had_rejections=false` (because the
/// rejection check short-circuits on `stored.is_empty()`), the solver
/// falls back to `[0,0,0]`, `build_clean` simulates with those zeros, and
/// `eq_derived` matches the original (skills don't actually update — a
/// separate apply-evaluator bug masks the mismatch). User is bypassed.
///
/// Contract: rebuild must NOT silently commit when a pending interactive
/// feat has no usable stored inputs in `original`. Either
/// `had_rejections=true` (cleanest signal) or the simulated character must
/// differ from `original` in derived state.
#[wasm_bindgen_test]
async fn rebuild_does_not_silently_commit_when_pending_feat_has_empty_inputs() {
    let _ = any_spawner::Executor::init_wasm_bindgen();
    let owner = Owner::new();
    owner.set();

    let registry = build_registry();
    registry.await_ready().await;

    let character = character_after_stage2_partial_rebuild();
    let original = character.clone();
    let preview = prepare_rebuild(character, &registry).expect("prepare_rebuild");

    // Sanity: the BA(Criminal) row should be surfaced as pending — without
    // it the test is meaningless.
    let pending_names: Vec<String> = preview
        .pending
        .iter()
        .map(|pi| pi.feature_name.to_string())
        .collect();
    assert!(
        pending_names
            .iter()
            .any(|name| name == "Background Abilities (Criminal)"),
        "test setup: BA(Criminal) must be pending; got {pending_names:?}",
    );

    let guessed = synthesize_apply_inputs(&preview.pending);
    let plan = level_up_plan(&original.core, &registry).expect("level_up_plan must succeed");
    let outcome = build_clean(&original, &plan, &registry, &guessed)
        .expect("build_clean must succeed with synthesized inputs");
    let simulated = outcome.character;

    let silent_commit_would_fire = !preview.had_rejections && simulated.eq_derived(&original);

    assert!(
        !silent_commit_would_fire,
        "rebuild silently committed with solver fallback inputs — user was bypassed for \
         a feat with no usable stored inputs. had_rejections={had_rej}, \
         eq_derived={eq_derived}",
        had_rej = preview.had_rejections,
        eq_derived = simulated.eq_derived(&original),
    );
}

/// Documents evaluator contract: `;` inside one expression is an
/// all-or-nothing chain. If `guard` raises `Error::GuardFailed`, the entire
/// expression aborts and any post-`;` writes are skipped. Independent
/// effects belong in separate `assign` array elements — see
/// `split_assigns_let_skill_writes_survive_guard_failure` for the working
/// pattern.
#[wasm_bindgen_test]
fn multi_statement_expression_aborts_atomically_on_guard_failure() {
    use dnd_pc::model::ProficiencyLevel;

    let expr_str = "with(@ABILITY(STR, DEX, CHA), guard(fold(and, @, in(@ARG, 0, 2) and @ + @ARG <= 20) and fold(+, @, @ARG) == 3, each(@, if(@ < 20, @ += @ARG)))); SKILL.ACRO.PROF = 1; SKILL.PERF.PROF = 1";
    let expr: Expr = expr_str.parse().expect("expression parses");
    let assignments = [Assignment {
        expr,
        when: WhenCondition::OnFeatureAdd,
    }];

    let mut character = Character::default();
    character.features.list.push(Feature {
        name: "Test Background Ability".into(),
        source: FeatureSource::Background("Test".into()),
        applied: true,
        category: FeatureCategory::Origin,
        inputs: Vec::new(),
        ..Feature::default()
    });

    // ARGs fail the guard (sum != 3). The `each` body is skipped (good),
    // and `guard` raises `Err(GuardFailed)` which aborts the whole
    // expression — the SKILL.* writes after `;` never execute.
    let inputs = [AssignInputs {
        args: vec![0, 0, 0],
        ..AssignInputs::default()
    }];

    apply_assignments_with_inputs(
        &mut character.core,
        &assignments,
        WhenCondition::OnFeatureAdd,
        &inputs,
        true,
    );

    // Abilities unchanged: guard prevented the `each` body.
    assert_eq!(character.abilities.strength, 8);
    // Skills also unchanged: the `Err(GuardFailed)` from `guard` aborted the
    // entire expression, so the SKILL.* writes after `;` did NOT run. This
    // is the correct atomic semantics for `;` inside one expression.
    assert_eq!(
        character.skills.get(Skill::Acrobatics),
        ProficiencyLevel::None,
    );
    assert_eq!(
        character.skills.get(Skill::Performance),
        ProficiencyLevel::None,
    );
}

/// Mirror of the test above with the assigns split at the natural failure
/// boundary: the ASI step (guard-protected, can fail) lives in its own
/// element; the SKILL.* writes share one expression because they're
/// unconditional and tightly coupled. `apply_assignments_with_inputs`
/// catches the `Err(GuardFailed)` from the ASI step and continues to the
/// SKILL.* step, where both proficiencies land.
#[wasm_bindgen_test]
fn split_assigns_let_skill_writes_survive_guard_failure() {
    use dnd_pc::model::ProficiencyLevel;

    let asi_expr: Expr = "with(@ABILITY(STR, DEX, CHA), guard(fold(and, @, in(@ARG, 0, 2) and @ + @ARG <= 20) and fold(+, @, @ARG) == 3, each(@, if(@ < 20, @ += @ARG))))".parse().expect("parses");
    let skills_expr: Expr = "SKILL.ACRO.PROF = 1; SKILL.PERF.PROF = 1"
        .parse()
        .expect("parses");

    let assignments = [
        Assignment {
            expr: asi_expr,
            when: WhenCondition::OnFeatureAdd,
        },
        Assignment {
            expr: skills_expr,
            when: WhenCondition::OnFeatureAdd,
        },
    ];

    let mut character = Character::default();
    character.features.list.push(Feature {
        name: "Test Background Ability".into(),
        source: FeatureSource::Background("Test".into()),
        applied: true,
        category: FeatureCategory::Origin,
        inputs: Vec::new(),
        ..Feature::default()
    });

    let inputs = [AssignInputs {
        args: vec![0, 0, 0],
        ..AssignInputs::default()
    }];

    apply_assignments_with_inputs(
        &mut character.core,
        &assignments,
        WhenCondition::OnFeatureAdd,
        &inputs,
        true,
    );

    // ASI step failed silently (logged, not propagated) — abilities stayed
    // at Character::default's STR=8.
    assert_eq!(character.abilities.strength, 8);

    // SKILL.* steps are independent assigns — they ran regardless of the
    // ASI step's outcome. This is the expected end-state after splitting
    // the def's `assign` array.
    assert_eq!(
        character.skills.get(Skill::Acrobatics),
        ProficiencyLevel::Proficient,
    );
    assert_eq!(
        character.skills.get(Skill::Performance),
        ProficiencyLevel::Proficient,
    );
}
