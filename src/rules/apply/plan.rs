use std::collections::{BTreeMap, BTreeSet};

use crate::{
    model::{
        CharacterCore, CharacterIdentity, ClassLevel, Feature, FeatureCategory, FeatureSource,
        Features, IdentitySlot,
    },
    rules::{
        FeaturesView, RulesRegistry,
        apply::{
            DefinitionCaches,
            pending::{
                ApplyInputs, InputsForFn, PICK_BACKGROUND, PICK_CLASS, PICK_SPECIES, PICK_SUBCLASS,
                PendingFeature, ReplacementForFn,
            },
            primitives::cascade,
            rebuild::{DefinitionKind, RebuildError, make_inputs_for, make_replacement_for},
        },
        class::ClassDefinition,
    },
};

/// Build the canonical level-up plan from `original`. Prefers the marker
/// path; falls back to interleaving for legacy chars without markers.
pub fn level_up_plan(
    original: &CharacterCore,
    registry: &RulesRegistry,
) -> Result<Vec<PendingFeature>, RebuildError> {
    if let Some(plan) = plan_from_markers(original) {
        return Ok(plan);
    }
    plan_from_interleaving(original, registry)
}

/// Build a plan straight from the System(_) markers in `features.list`.
/// Returns `None` when no class markers exist (legacy chars) — caller
/// falls back to interleaving.
pub fn plan_from_markers(original: &CharacterCore) -> Option<Vec<PendingFeature>> {
    let identity = &original.identity;
    let features = &original.features;
    // Group System(Class) markers by class name and check coverage.
    let mut class_markers: BTreeMap<Box<str>, Vec<&Feature>> = BTreeMap::new();
    for feature in features.iter() {
        if matches!(
            feature.category,
            FeatureCategory::System(IdentitySlot::Class)
        ) {
            class_markers
                .entry(feature.name.clone())
                .or_default()
                .push(feature);
        }
    }

    // Markers are the source of truth: identity.classes is a denormalized
    // cache. If markers disagree with identity (typically because a buggy
    // earlier replay double-stacked CLASS.LEVEL), trust the markers — the
    // walker will rebuild identity.classes from them. Sort each class's
    // markers by their User(N) for the per-level walk below.
    let total_markers: u32 = class_markers
        .values()
        .map(|markers| markers.len() as u32)
        .sum();
    for markers in class_markers.values_mut() {
        markers.sort_by_key(|feature| feature.source.added_at_level());
    }

    // All markers must live on User(_) sources.
    if class_markers
        .values()
        .flat_map(|markers| markers.iter())
        .any(|feature| !feature.source.is_user())
    {
        return None;
    }

    // Build User(N) → marker map and require contiguous 1..=total_markers.
    let mut by_user_level: BTreeMap<u32, &Feature> = BTreeMap::new();
    for markers in class_markers.values() {
        for feature in markers {
            by_user_level.insert(feature.source.added_at_level(), feature);
        }
    }
    if total_markers > 0 {
        for level in 1..=total_markers {
            if !by_user_level.contains_key(&level) {
                return None;
            }
        }
    }
    // No System(Class) markers: either a fresh char (no classes anywhere)
    // or a legacy char (classes in identity but no markers yet). Fall
    // through to interleaving — it'll synthesize markers from identity
    // and bring the char into the marker-driven world on first rebuild.
    if total_markers == 0 {
        return None;
    }

    let mut plan: Vec<PendingFeature> = Vec::new();

    // 1. User(0) non-System feats (preserve original order).
    emit_user_features(features, &mut plan, 0);

    // 2. System(Species) and System(Background) markers — preferring
    // existing markers, synthesizing from identity for legacy half-state.
    push_identity_marker(identity, features, &mut plan, IdentitySlot::Species);
    push_identity_marker(identity, features, &mut plan, IdentitySlot::Background);

    // 4. For each character level: System(Class) marker, optional
    // System(Subclass) marker (matched by Class(name, class_level) source),
    // then User(N) non-System feats.
    let mut class_running: BTreeMap<Box<str>, u32> = BTreeMap::new();
    for character_level in 1..=total_markers {
        let class_marker = by_user_level
            .get(&character_level)
            .copied()
            .expect("contiguous coverage validated above");
        let class_level = *class_running
            .entry(class_marker.name.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        plan.push(PendingFeature {
            name: class_marker.name.clone(),
            source: class_marker.source.clone(),
            level: character_level,
            replaces: Some(PICK_CLASS.into()),
        });

        // Subclass marker for this class+level, if present.
        let subclass_match = features.iter().find(|feature| {
            matches!(
                feature.category,
                FeatureCategory::System(IdentitySlot::Subclass)
            ) && matches!(
                &feature.source,
                FeatureSource::Class(class_name, level)
                    if class_name.as_ref() == &*class_marker.name
                        && *level == class_level
            )
        });
        if let Some(subclass) = subclass_match {
            plan.push(PendingFeature {
                name: subclass.name.clone(),
                source: subclass.source.clone(),
                level: class_level,
                replaces: Some(PICK_SUBCLASS.into()),
            });
        }

        emit_user_features(features, &mut plan, character_level);
    }

    Some(plan)
}

/// Emit the marker + `replaces=Some(placeholder)` so cascade records the
/// swap. Empty identity emits the placeholder so the modal asks.
fn push_identity_marker(
    identity: &CharacterIdentity,
    features: &Features,
    plan: &mut Vec<PendingFeature>,
    slot: IdentitySlot,
) {
    let (identity_name, placeholder) = match slot {
        IdentitySlot::Species => (&identity.species, PICK_SPECIES),
        IdentitySlot::Background => (&identity.background, PICK_BACKGROUND),
        IdentitySlot::Class | IdentitySlot::Subclass => return,
    };
    let marker = find_system_marker(features, slot);
    let (name, source, replaces): (Box<str>, FeatureSource, Option<Box<str>>) =
        match (marker, identity_name.is_empty()) {
            (Some(marker), _) => (
                marker.name.clone(),
                marker.source.clone(),
                Some(placeholder.into()),
            ),
            (None, false) => (
                identity_name.as_str().into(),
                FeatureSource::User(0),
                Some(placeholder.into()),
            ),
            (None, true) => (placeholder.into(), FeatureSource::User(0), None),
        };
    plan.push(PendingFeature {
        name,
        source,
        level: 0,
        replaces,
    });
}

fn find_system_marker(features: &Features, slot: IdentitySlot) -> Option<&Feature> {
    features
        .iter()
        .find(|feature| feature.category == FeatureCategory::System(slot))
}

/// Run the legacy interleaving algorithm on a throwaway character and emit a
/// canonical `Vec<PendingFeature>` of System markers + User feats.
pub fn plan_from_interleaving(
    original: &CharacterCore,
    registry: &RulesRegistry,
) -> Result<Vec<PendingFeature>, RebuildError> {
    registry.with_definitions(|caches| {
        registry.with_features_index_untracked(|feat_index| {
            plan_from_interleaving_with_caches(original, feat_index, caches)
        })
    })
}

/// Caches-explicit form of [`plan_from_interleaving`]. Kept as a separate
/// function so unit tests can supply hand-built caches without spinning up a
/// real [`RulesRegistry`].
pub fn plan_from_interleaving_with_caches(
    original: &CharacterCore,
    feat_index: FeaturesView<'_>,
    caches: DefinitionCaches,
) -> Result<Vec<PendingFeature>, RebuildError> {
    let identity = &original.identity;
    let features = &original.features;
    let mut plan: Vec<PendingFeature> = Vec::new();

    // Throwaway character used solely to evaluate multiclass prereqs against
    // a realistic post-species/background ability state. The cascade walks
    // below replay each marker + its identity-event follow-ups with the
    // user's stored inputs (via `inputs_for`), so probe abilities reflect
    // each level's accumulated ASIs etc. — not CharacterCore::default's
    // zeros.
    //
    // Identity slots start CLEARED so markers fire `IdentityChange` events
    // when their assigns set the slot — `ApplyContext::assign` suppresses
    // the event if the slot already matches, which would silently skip
    // species/background/class follow-ups (incl. ASI bumps) on probe.
    let probe_classes: Vec<ClassLevel> = identity
        .classes
        .iter()
        .map(|class_level| ClassLevel {
            class: class_level.class.clone(),
            subclass: class_level.subclass.clone(),
            level: 0,
            ..ClassLevel::default()
        })
        .collect();
    let mut probe = CharacterCore {
        identity: CharacterIdentity {
            classes: probe_classes,
            ..CharacterIdentity::default()
        },
        ..CharacterCore::default()
    };

    let extra_inputs = ApplyInputs::default();
    let pending_keys: BTreeSet<(&str, &FeatureSource)> = BTreeSet::new();
    let inputs_for = make_inputs_for(feat_index, original, &extra_inputs);
    let replacement_for = make_replacement_for(feat_index, original, &extra_inputs, &pending_keys);

    // 1. User(0) non-System feats (e.g. Generation: * which sets the base
    //    abilities). Emit into plan AND cascade onto probe so abilities seeded by
    //    Generation are visible to subsequent prereq checks.
    emit_and_apply_user_features(
        features,
        &mut plan,
        0,
        feat_index,
        caches,
        &inputs_for,
        &replacement_for,
        &mut probe,
    );

    // 2. System(Species) marker + cascade simulates follow-ups on probe.
    let species_marker_idx = plan.len();
    push_identity_marker(identity, features, &mut plan, IdentitySlot::Species);
    if let Some(marker) = plan.get(species_marker_idx).cloned()
        && !identity.species.is_empty()
    {
        // Force the missing-definition check up-front so callers see a
        // precise error instead of a silent cascade no-op.
        caches
            .species
            .get(identity.species.as_str())
            .ok_or_else(|| RebuildError::MissingDefinition {
                kind: DefinitionKind::Species,
                name: identity.species.as_str().into(),
            })?;
        cascade(
            &mut probe,
            std::slice::from_ref(&marker),
            feat_index,
            caches,
            &inputs_for,
            &replacement_for,
            false,
        );
    }

    // 3. System(Background) marker — symmetric.
    let bg_marker_idx = plan.len();
    push_identity_marker(identity, features, &mut plan, IdentitySlot::Background);
    if let Some(marker) = plan.get(bg_marker_idx).cloned()
        && !identity.background.is_empty()
    {
        caches
            .backgrounds
            .get(identity.background.as_str())
            .ok_or_else(|| RebuildError::MissingDefinition {
                kind: DefinitionKind::Background,
                name: identity.background.as_str().into(),
            })?;
        cascade(
            &mut probe,
            std::slice::from_ref(&marker),
            feat_index,
            caches,
            &inputs_for,
            &replacement_for,
            false,
        );
    }

    // 4. Class levels — interleaved with multiclass prereq re-evaluation.
    apply_classes_interleaved(
        identity,
        features,
        feat_index,
        caches,
        &inputs_for,
        &replacement_for,
        &mut probe,
        &mut plan,
    )?;

    Ok(plan)
}

/// Walk class levels in canonical order (primary first, multiclasses gated by
/// prereq each iteration), appending System markers + User(N) feats to `plan`.
/// `probe` is the throwaway character used for prereq checks; it ends in an
/// implementation-detail state and must be discarded.
#[allow(clippy::too_many_arguments)]
fn apply_classes_interleaved(
    identity: &CharacterIdentity,
    features: &Features,
    feat_index: FeaturesView<'_>,
    caches: DefinitionCaches,
    inputs_for: &InputsForFn<'_>,
    replacement_for: &ReplacementForFn<'_>,
    probe: &mut CharacterCore,
    plan: &mut Vec<PendingFeature>,
) -> Result<(), RebuildError> {
    let n_classes = identity.classes.len();
    if n_classes == 0 {
        return Ok(());
    }
    let targets: Vec<u32> = identity
        .classes
        .iter()
        .map(|class_level| class_level.level)
        .collect();
    if targets.iter().all(|&t| t == 0) {
        return Ok(());
    }

    let mut applied: Vec<u32> = vec![0; n_classes];
    let mut character_level: u32 = 0;

    if targets[0] > 0 && !identity.classes[0].class.is_empty() {
        emit_class_level(
            identity,
            feat_index,
            caches,
            inputs_for,
            replacement_for,
            probe,
            plan,
            0,
            1,
        )?;
        applied[0] = 1;
        character_level = 1;
        emit_and_apply_user_features(
            features,
            plan,
            character_level,
            feat_index,
            caches,
            inputs_for,
            replacement_for,
            probe,
        );
    }

    while let Some(idx) = pick_next_class(probe, &targets, &applied, caches.classes) {
        let next_class_lvl = applied[idx] + 1;
        emit_class_level(
            identity,
            feat_index,
            caches,
            inputs_for,
            replacement_for,
            probe,
            plan,
            idx,
            next_class_lvl,
        )?;
        applied[idx] = next_class_lvl;
        character_level += 1;
        emit_and_apply_user_features(
            features,
            plan,
            character_level,
            feat_index,
            caches,
            inputs_for,
            replacement_for,
            probe,
        );
    }

    applied
        .iter()
        .zip(&targets)
        .enumerate()
        .skip(1)
        .find(|(idx, (applied, target))| {
            applied < target && !identity.classes[*idx].class.is_empty()
        })
        .map_or(Ok(()), |(idx, _)| {
            Err(RebuildError::MulticlassPrereq {
                class: identity.classes[idx].class.clone(),
            })
        })
}

/// Pick the next class to level. Mirrors the legacy primary-first / prereq-
/// gated multiclass logic, evaluated against `probe`'s current state.
fn pick_next_class(
    probe: &CharacterCore,
    targets: &[u32],
    applied: &[u32],
    classes: &BTreeMap<Box<str>, ClassDefinition>,
) -> Option<usize> {
    let meets_prereq = |class_name: &str| {
        classes
            .get(class_name)
            .is_none_or(|class_def| class_def.meets_prerequisites(probe))
    };

    let idx = if meets_prereq(probe.identity.classes[0].class.as_ref()) {
        probe
            .identity
            .classes
            .iter()
            .enumerate()
            .skip(1)
            .find(|(idx, class_level)| {
                !class_level.class.is_empty()
                    && applied[*idx] < targets[*idx]
                    && meets_prereq(class_level.class.as_ref())
            })
            .map_or(0, |(idx, _)| idx)
    } else {
        0
    };

    let class_level = &probe.identity.classes[idx];
    (!class_level.class.is_empty() && applied[idx] < targets[idx]).then_some(idx)
}

/// Append a System(Class) marker for `class_idx`, optionally a System(Subclass)
/// marker if this is the subclass-pick level, then advance the probe character
/// so subsequent prereq checks see post-ASI ability scores.
#[allow(clippy::too_many_arguments)]
fn emit_class_level(
    identity: &CharacterIdentity,
    feat_index: FeaturesView<'_>,
    caches: DefinitionCaches,
    inputs_for: &InputsForFn<'_>,
    replacement_for: &ReplacementForFn<'_>,
    probe: &mut CharacterCore,
    plan: &mut Vec<PendingFeature>,
    class_idx: usize,
    class_level: u32,
) -> Result<(), RebuildError> {
    let class_name = identity.classes[class_idx].class.as_ref();
    let class_def =
        caches
            .classes
            .get(class_name)
            .ok_or_else(|| RebuildError::MissingDefinition {
                kind: DefinitionKind::Class,
                name: class_name.into(),
            })?;

    // Total character level after this step = (previously-applied levels
    // across all classes) + 1. Used to source the System(Class) marker.
    let character_level: u32 = probe
        .identity
        .classes
        .iter()
        .map(|class_level| class_level.level)
        .sum::<u32>()
        + 1;

    let start = plan.len();
    plan.push(PendingFeature {
        name: Box::from(class_name),
        source: FeatureSource::User(character_level),
        level: character_level,
        replaces: Some(PICK_CLASS.into()),
    });

    // Subclass marker fires at the lowest level the subclass declares
    // features for. Pushed alongside the class marker so a single cascade
    // call walks both in order — class identity event drives class L_n
    // follow-ups, then subclass event drives subclass L_n follow-ups.
    if let Some(subclass_name) = identity.classes[class_idx].subclass.as_deref()
        && let Some(subclass_def) = class_def.subclasses.get(subclass_name)
    {
        let pick_level = subclass_def.levels.keys().next().copied().unwrap_or(0);
        if pick_level == class_level {
            plan.push(PendingFeature {
                name: Box::from(subclass_name),
                source: FeatureSource::Class(class_name.into(), class_level),
                level: class_level,
                replaces: Some(PICK_SUBCLASS.into()),
            });
        }
    }

    cascade(
        probe,
        &plan[start..],
        feat_index,
        caches,
        inputs_for,
        replacement_for,
        false,
    );

    Ok(())
}

/// Append non-System User(level) feats from `features` after the level's
/// class marker — preserves original feature-list ordering.
fn emit_user_features(features: &Features, plan: &mut Vec<PendingFeature>, level: u32) {
    for feature in features.iter() {
        if !matches!(&feature.source, FeatureSource::User(l) if *l == level) {
            continue;
        }
        if matches!(feature.category, FeatureCategory::System(_)) {
            continue;
        }
        plan.push(PendingFeature {
            name: feature.name.clone(),
            source: feature.source.clone(),
            level,
            replaces: None,
        });
    }
}

/// Like `emit_user_features` but also cascades each emitted entry onto
/// `probe` so its abilities reflect what the user actually picked at this
/// level — e.g. a `Generation: *` feat at User(0) sets base ability scores,
/// without which subsequent multiclass prereq checks see `CharacterCore::
/// default` 8/8/8/8/8/8 instead of the character's real intermediate state.
#[allow(clippy::too_many_arguments)]
fn emit_and_apply_user_features(
    features: &Features,
    plan: &mut Vec<PendingFeature>,
    level: u32,
    feat_index: FeaturesView<'_>,
    caches: DefinitionCaches,
    inputs_for: &InputsForFn<'_>,
    replacement_for: &ReplacementForFn<'_>,
    probe: &mut CharacterCore,
) {
    let start = plan.len();
    emit_user_features(features, plan, level);
    cascade(
        probe,
        &plan[start..],
        feat_index,
        caches,
        inputs_for,
        replacement_for,
        false,
    );
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::*;
    use crate::model::{Character, Feature};

    fn marker(name: &str, slot: IdentitySlot, source: FeatureSource) -> Feature {
        Feature {
            name: name.into(),
            source,
            applied: true,
            category: FeatureCategory::System(slot),
            ..Feature::default()
        }
    }

    #[wasm_bindgen_test]
    fn plan_from_markers_preserves_class_order() {
        let mut original = Character::default();
        // Identity in arbitrary order — Wizard listed first to prove plan
        // walks marker User(N) sequence, not identity order.
        original.identity.classes = vec![
            ClassLevel {
                class: "Wizard".into(),
                level: 1,
                ..ClassLevel::default()
            },
            ClassLevel {
                class: "Fighter".into(),
                level: 2,
                ..ClassLevel::default()
            },
        ];
        original.features.list = vec![
            marker("Fighter", IdentitySlot::Class, FeatureSource::User(1)),
            marker("Wizard", IdentitySlot::Class, FeatureSource::User(2)),
            marker("Fighter", IdentitySlot::Class, FeatureSource::User(3)),
        ];

        let plan = plan_from_markers(&original.core).expect("plan");
        let class_entries: Vec<(&str, &FeatureSource)> = plan
            .iter()
            .filter(|pending| &*pending.name == "Fighter" || &*pending.name == "Wizard")
            .map(|pending| (&*pending.name, &pending.source))
            .collect();
        assert_eq!(
            class_entries,
            vec![
                ("Fighter", &FeatureSource::User(1)),
                ("Wizard", &FeatureSource::User(2)),
                ("Fighter", &FeatureSource::User(3)),
            ]
        );
    }

    #[wasm_bindgen_test]
    fn plan_from_markers_places_subclass_after_class_step() {
        let mut original = Character::default();
        original.identity.classes = vec![ClassLevel {
            class: "Fighter".into(),
            subclass: Some("Champion".into()),
            level: 3,
            ..ClassLevel::default()
        }];
        original.features.list = vec![
            marker("Fighter", IdentitySlot::Class, FeatureSource::User(1)),
            marker("Fighter", IdentitySlot::Class, FeatureSource::User(2)),
            marker("Fighter", IdentitySlot::Class, FeatureSource::User(3)),
            marker(
                "Champion",
                IdentitySlot::Subclass,
                FeatureSource::Class("Fighter".into(), 3),
            ),
        ];

        let plan = plan_from_markers(&original.core).expect("plan");
        // Find position of Fighter@User(3) and assert Champion sits next.
        let l3_pos = plan
            .iter()
            .position(|pending| {
                &*pending.name == "Fighter" && pending.source == FeatureSource::User(3)
            })
            .expect("L3 marker");
        let next = plan.get(l3_pos + 1).expect("after L3");
        assert_eq!(&*next.name, "Champion");
        assert_eq!(next.source, FeatureSource::Class("Fighter".into(), 3));
    }

    #[wasm_bindgen_test]
    fn plan_from_markers_trusts_markers_over_identity() {
        // When markers and identity disagree (e.g. a buggy earlier replay
        // double-stacked identity.classes.level), the plan trusts the
        // markers — they are the source of truth, identity is a cache.
        let mut original = Character::default();
        original.identity.classes = vec![ClassLevel {
            class: "Fighter".into(),
            level: 5, // identity claims L5 but only two markers exist
            ..ClassLevel::default()
        }];
        original.features.list = vec![
            marker("Fighter", IdentitySlot::Class, FeatureSource::User(1)),
            marker("Fighter", IdentitySlot::Class, FeatureSource::User(2)),
        ];

        let plan = plan_from_markers(&original.core).expect("plan");
        let class_steps = plan
            .iter()
            .filter(|pending| &*pending.name == "Fighter" && pending.source.is_user())
            .count();
        assert_eq!(
            class_steps, 2,
            "plan emits one step per marker, not per identity level"
        );
    }

    #[wasm_bindgen_test]
    fn plan_from_markers_falls_through_for_legacy_chars() {
        // No System(Class) markers means a legacy char (or fresh char) —
        // plan_from_markers must defer to interleaving so identity-only
        // chars still rebuild correctly.
        let mut original = Character::default();
        original.identity.species = "Human".into();
        original.identity.classes = vec![ClassLevel {
            class: "Fighter".into(),
            level: 3,
            ..ClassLevel::default()
        }];

        assert!(plan_from_markers(&original.core).is_none());
    }

    /// Empty species / background: the plan must include the
    /// `PICK_SPECIES` / `PICK_BACKGROUND` placeholders (replace_with:
    /// Category(System(_))) so the args modal can surface a picker during
    /// cascade.
    #[wasm_bindgen_test]
    fn plan_from_markers_emits_pick_placeholders_when_identity_empty() {
        let mut original = Character::default();
        // Class markers exist (so plan_from_markers doesn't fall through),
        // but species and background are blank.
        original.identity.classes = vec![ClassLevel {
            class: "Fighter".into(),
            level: 1,
            ..ClassLevel::default()
        }];
        original.features.list = vec![marker(
            "Fighter",
            IdentitySlot::Class,
            FeatureSource::User(1),
        )];

        let plan = plan_from_markers(&original.core).expect("plan");
        let names: Vec<&str> = plan.iter().map(|pending| &*pending.name).collect();
        assert!(
            names.contains(&PICK_SPECIES),
            "expected {PICK_SPECIES} placeholder for empty species, got {names:?}"
        );
        assert!(
            names.contains(&PICK_BACKGROUND),
            "expected {PICK_BACKGROUND} placeholder for empty background, got {names:?}"
        );
    }

    /// Diagnostic: cascade on a synth Background marker should set
    /// identity.background, fire identity event, and emit BG features.
    #[wasm_bindgen_test]
    fn cascade_applies_synth_bg_marker_and_emits_followups() {
        use crate::rules::{
            apply::{
                primitives::cascade,
                rebuild::{make_inputs_for, make_replacement_for},
            },
            background::BackgroundDefinition,
            class::ClassDefinition,
            feature::FeatureDefinition,
            registry::make_system_feature,
            species::SpeciesDefinition,
        };

        let original = Character::default();

        let mut feat_index_map: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index_map.insert(
            Box::from("Test BG"),
            make_system_feature(Box::from("Test BG"), IdentitySlot::Background),
        );
        let feat_index = FeaturesView::from_natural(&feat_index_map);

        let class_defs: BTreeMap<Box<str>, ClassDefinition> = BTreeMap::new();
        let species_defs: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_defs: BTreeMap<Box<str>, BackgroundDefinition> = [(
            Box::<str>::from("Test BG"),
            BackgroundDefinition {
                name: Box::from("Test BG"),
                features: ["Sub Feat".to_string()].into_iter().collect(),
            },
        )]
        .into_iter()
        .collect();
        let caches = DefinitionCaches {
            classes: &class_defs,
            species: &species_defs,
            backgrounds: &bg_defs,
        };

        let extra_inputs = ApplyInputs::default();
        let pending_keys: BTreeSet<(&str, &FeatureSource)> = BTreeSet::new();
        let inputs_for = make_inputs_for(feat_index, &original.core, &extra_inputs);
        let replacement_for =
            make_replacement_for(feat_index, &original.core, &extra_inputs, &pending_keys);

        let mut probe = CharacterCore::default();
        let pending = vec![PendingFeature {
            name: "Test BG".into(),
            source: FeatureSource::User(0),
            level: 0,
            replaces: Some(PICK_BACKGROUND.into()),
        }];
        cascade(
            &mut probe,
            &pending,
            feat_index,
            caches,
            &inputs_for,
            &replacement_for,
            false,
        );

        let features: Vec<(String, FeatureSource)> = probe
            .features
            .iter()
            .map(|f| (f.name.to_string(), f.source.clone()))
            .collect();
        assert_eq!(
            probe.identity.background.as_str(),
            "Test BG",
            "marker assign should set identity.background; got features={features:?}"
        );
        assert!(
            probe.applied.background,
            "identity event should flip applied.background; got features={features:?}"
        );
    }

    /// Sanity: cascade applies a non-stackable feat whose `@ABIL(DEX,
    /// CON, CHA)` group-iter assign bumps abilities from stored args.
    #[wasm_bindgen_test]
    fn cascade_applies_bg_abilities_group_assign_to_probe() {
        use crate::{
            model::{AssignInputs, Expr},
            rules::{
                ReplaceWith, WhenCondition,
                apply::{
                    primitives::cascade,
                    rebuild::{make_inputs_for, make_replacement_for},
                },
                background::BackgroundDefinition,
                class::ClassDefinition,
                feature::{Assignment, FeatureDefinition},
                species::SpeciesDefinition,
            },
        };

        let mut original = Character::default();
        original.features.list.push(Feature {
            name: "Test BG Abilities".into(),
            source: FeatureSource::Background("Test BG".into()),
            applied: true,
            category: FeatureCategory::Origin,
            inputs: vec![AssignInputs {
                args: vec![1, 0, 2],
                ..AssignInputs::default()
            }],
            ..Feature::default()
        });

        let mut feat_index_map: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index_map.insert(
            Box::from("Test BG Abilities"),
            FeatureDefinition {
                name: Box::from("Test BG Abilities"),
                stackable: false,
                category: FeatureCategory::Origin,
                replace_with: ReplaceWith::None,
                spells: None,
                actions: BTreeMap::new(),
                assign: Some(vec![Assignment {
                    expr: "with(@ABIL(DEX, CON, CHA), guard(fold(and, @, in(@ARG, 0, 2) and @ + @ARG <= 20) and fold(+, @, @ARG) == 3, each(@, if(@ < 20, @ += @ARG))))"
                        .parse::<Expr>()
                        .unwrap(),
                    when: WhenCondition::OnFeatureAdd,
                }]),
                prerequisites: None,
            },
        );
        let feat_index = FeaturesView::from_natural(&feat_index_map);
        let class_defs: BTreeMap<Box<str>, ClassDefinition> = BTreeMap::new();
        let species_defs: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_defs: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_defs,
            species: &species_defs,
            backgrounds: &bg_defs,
        };

        let extra_inputs = ApplyInputs::default();
        let pending_keys: BTreeSet<(&str, &FeatureSource)> = BTreeSet::new();
        let inputs_for = make_inputs_for(feat_index, &original.core, &extra_inputs);
        let replacement_for =
            make_replacement_for(feat_index, &original.core, &extra_inputs, &pending_keys);

        let mut probe = CharacterCore {
            abilities: crate::model::AbilityScores {
                strength: 10,
                dexterity: 15,
                constitution: 16,
                intelligence: 13,
                wisdom: 11,
                charisma: 9,
            },
            ..CharacterCore::default()
        };
        let pending = vec![PendingFeature {
            name: "Test BG Abilities".into(),
            source: FeatureSource::Background("Test BG".into()),
            level: 0,
            replaces: None,
        }];
        cascade(
            &mut probe,
            &pending,
            feat_index,
            caches,
            &inputs_for,
            &replacement_for,
            false,
        );

        // args=[1,0,2] over @ABIL(DEX, CON, CHA): DEX+1, CON+0, CHA+2.
        assert_eq!(probe.abilities.dexterity, 16);
        assert_eq!(probe.abilities.constitution, 16);
        assert_eq!(probe.abilities.charisma, 11);
    }

    /// Sanity: cascade applies a dice-driven Generation feat on probe
    /// with stored dice, abilities should reflect rolled values.
    #[wasm_bindgen_test]
    fn cascade_applies_dice_generation_to_probe() {
        use crate::{
            model::{AssignInputs, Expr},
            rules::{
                ReplaceWith, WhenCondition,
                apply::{
                    primitives::cascade,
                    rebuild::{make_inputs_for, make_replacement_for},
                },
                background::BackgroundDefinition,
                class::ClassDefinition,
                feature::{Assignment, FeatureDefinition},
                species::SpeciesDefinition,
            },
        };

        let mut original = Character::default();
        let mut dice = BTreeMap::<u32, Vec<u32>>::new();
        dice.insert(
            6,
            vec![
                3, 5, 2, 2, 2, 6, 6, 3, 6, 4, 6, 1, 1, 4, 3, 6, 4, 3, 4, 2, 2, 2, 1, 5,
            ],
        );
        original.features.list.push(Feature {
            name: "Generation: Test".into(),
            source: FeatureSource::User(0),
            applied: true,
            category: FeatureCategory::Generation,
            inputs: vec![AssignInputs {
                args: vec![],
                dice: dice.into(),
            }],
            ..Feature::default()
        });

        let mut feat_index_map: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        feat_index_map.insert(
            Box::from("Generation: Test"),
            FeatureDefinition {
                name: Box::from("Generation: Test"),
                stackable: false,
                category: FeatureCategory::Generation,
                replace_with: ReplaceWith::None,
                spells: None,
                actions: BTreeMap::new(),
                assign: Some(vec![Assignment {
                    expr: "STR = 4d6kh3; DEX = 4d6kh3; CON = 4d6kh3; INT = 4d6kh3; WIS = 4d6kh3; CHA = 4d6kh3"
                        .parse::<Expr>()
                        .unwrap(),
                    when: WhenCondition::OnFeatureAdd,
                }]),
                prerequisites: None,
            },
        );
        let feat_index = FeaturesView::from_natural(&feat_index_map);
        let class_defs: BTreeMap<Box<str>, ClassDefinition> = BTreeMap::new();
        let species_defs: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_defs: BTreeMap<Box<str>, BackgroundDefinition> = BTreeMap::new();
        let caches = DefinitionCaches {
            classes: &class_defs,
            species: &species_defs,
            backgrounds: &bg_defs,
        };

        let extra_inputs = ApplyInputs::default();
        let pending_keys: BTreeSet<(&str, &FeatureSource)> = BTreeSet::new();
        let inputs_for = make_inputs_for(feat_index, &original.core, &extra_inputs);
        let replacement_for =
            make_replacement_for(feat_index, &original.core, &extra_inputs, &pending_keys);

        let mut probe = CharacterCore::default();
        let pending = vec![PendingFeature {
            name: "Generation: Test".into(),
            source: FeatureSource::User(0),
            level: 0,
            replaces: None,
        }];
        cascade(
            &mut probe,
            &pending,
            feat_index,
            caches,
            &inputs_for,
            &replacement_for,
            false,
        );

        // 4d6kh3 from dice: STR=5+3+2=10, DEX=6+6+3=15, CON=6+6+4=16,
        // INT=6+4+3=13, WIS=4+4+3=11, CHA=5+2+2=9.
        assert_eq!(probe.abilities.strength, 10, "STR after dice gen");
        assert_eq!(probe.abilities.dexterity, 15, "DEX after dice gen");
        assert_eq!(probe.abilities.constitution, 16, "CON after dice gen");
        assert_eq!(probe.abilities.intelligence, 13, "INT after dice gen");
        assert_eq!(probe.abilities.wisdom, 11, "WIS after dice gen");
        assert_eq!(probe.abilities.charisma, 9, "CHA after dice gen");
    }

    /// Legacy multiclass rebuild: a User(0) Generation feat sets base
    /// abilities; a Monk L8 ASI follow-up bumps CHA the last +2 to satisfy
    /// Bard's `CHA >= 13` prereq. Probe walking must replay both stages.
    #[wasm_bindgen_test]
    fn plan_from_interleaving_succeeds_for_legacy_multiclass_with_satisfied_prereqs() {
        use crate::{
            model::{AssignInputs, Expr},
            rules::{
                ReplaceWith, WhenCondition,
                background::BackgroundDefinition,
                class::ClassDefinition,
                feature::{Assignment, FeatureDefinition},
                registry::make_system_feature,
                species::SpeciesDefinition,
            },
        };

        let mut original = Character::default();
        original.identity.classes = vec![
            ClassLevel {
                class: "Monk".into(),
                level: 8,
                ..ClassLevel::default()
            },
            ClassLevel {
                class: "Bard".into(),
                level: 1,
                ..ClassLevel::default()
            },
        ];
        original.identity.background = "Test BG".into();

        // Pre-rolled d6 pool consumed in order by the 6 `4d6kh3` statements:
        // STR=10, DEX=15, CON=16, INT=13, WIS=11, CHA=9.
        let mut dice = BTreeMap::<u32, Vec<u32>>::new();
        dice.insert(
            6,
            vec![
                3, 5, 2, 2, 2, 6, 6, 3, 6, 4, 6, 1, 1, 4, 3, 6, 4, 3, 4, 2, 2, 2, 1, 5,
            ],
        );
        original.features.list.push(Feature {
            name: "Generation: Test".into(),
            source: FeatureSource::User(0),
            applied: true,
            category: FeatureCategory::Generation,
            inputs: vec![AssignInputs {
                args: vec![],
                dice: dice.into(),
            }],
            ..Feature::default()
        });

        // Background bumps DEX+1, CHA+2 — taking CHA from 9 to 11.
        original.features.list.push(Feature {
            name: "Test BG Abilities".into(),
            source: FeatureSource::Background("Test BG".into()),
            applied: true,
            category: FeatureCategory::Origin,
            inputs: vec![AssignInputs {
                args: vec![1, 0, 2],
                ..AssignInputs::default()
            }],
            ..Feature::default()
        });

        original.features.list.push(Feature {
            name: "Test ASI".into(),
            source: FeatureSource::Class("Monk".into(), 8),
            applied: true,
            category: FeatureCategory::General,
            inputs: vec![AssignInputs {
                args: vec![0, 0, 0, 0, 0, 2],
                ..AssignInputs::default()
            }],
            ..Feature::default()
        });

        let monk_def: ClassDefinition = serde_json::from_value(serde_json::json!({
            "name": "Monk",
            "levels": {
                "1": { "features": [] },
                "2": { "features": [] },
                "3": { "features": [] },
                "4": { "features": [] },
                "5": { "features": [] },
                "6": { "features": [] },
                "7": { "features": [] },
                "8": { "features": ["Test ASI"] }
            }
        }))
        .unwrap();
        let bard_def: ClassDefinition = serde_json::from_value(serde_json::json!({
            "name": "Bard",
            "prerequisites": "CHA >= 13",
            "levels": { "1": { "features": [] } }
        }))
        .unwrap();
        let class_defs: BTreeMap<Box<str>, ClassDefinition> =
            [(Box::from("Monk"), monk_def), (Box::from("Bard"), bard_def)]
                .into_iter()
                .collect();
        let species_defs: BTreeMap<Box<str>, SpeciesDefinition> = BTreeMap::new();
        let bg_defs: BTreeMap<Box<str>, BackgroundDefinition> = [(
            Box::<str>::from("Test BG"),
            BackgroundDefinition {
                name: Box::from("Test BG"),
                features: ["Test BG Abilities".to_string()].into_iter().collect(),
            },
        )]
        .into_iter()
        .collect();
        let caches = DefinitionCaches {
            classes: &class_defs,
            species: &species_defs,
            backgrounds: &bg_defs,
        };

        let mut feat_index_map: BTreeMap<Box<str>, FeatureDefinition> = BTreeMap::new();
        for name in ["Monk", "Bard"] {
            feat_index_map.insert(
                Box::from(name),
                make_system_feature(Box::from(name), IdentitySlot::Class),
            );
        }
        feat_index_map.insert(
            Box::from("Test BG"),
            make_system_feature(Box::from("Test BG"), IdentitySlot::Background),
        );
        feat_index_map.insert(
            Box::from("Test BG Abilities"),
            FeatureDefinition {
                name: Box::from("Test BG Abilities"),
                stackable: false,
                category: FeatureCategory::Origin,
                replace_with: ReplaceWith::None,
                spells: None,
                actions: BTreeMap::new(),
                assign: Some(vec![Assignment {
                    expr: "with(@ABIL(DEX, CON, CHA), guard(fold(and, @, in(@ARG, 0, 2) and @ + @ARG <= 20) and fold(+, @, @ARG) == 3, each(@, if(@ < 20, @ += @ARG))))"
                        .parse::<Expr>()
                        .unwrap(),
                    when: WhenCondition::OnFeatureAdd,
                }]),
                prerequisites: None,
            },
        );
        feat_index_map.insert(
            Box::from("Generation: Test"),
            FeatureDefinition {
                name: Box::from("Generation: Test"),
                stackable: false,
                category: FeatureCategory::Generation,
                replace_with: ReplaceWith::None,
                spells: None,
                actions: BTreeMap::new(),
                assign: Some(vec![Assignment {
                    expr: "STR = 4d6kh3; DEX = 4d6kh3; CON = 4d6kh3; INT = 4d6kh3; WIS = 4d6kh3; CHA = 4d6kh3"
                        .parse::<Expr>()
                        .unwrap(),
                    when: WhenCondition::OnFeatureAdd,
                }]),
                prerequisites: None,
            },
        );
        feat_index_map.insert(
            Box::from("Test ASI"),
            FeatureDefinition {
                name: Box::from("Test ASI"),
                stackable: false,
                category: FeatureCategory::General,
                replace_with: ReplaceWith::None,
                spells: None,
                actions: BTreeMap::new(),
                assign: Some(vec![Assignment {
                    expr: "with(@ABIL, guard(fold(and, @, in(@ARG, 0, 2) and @ + @ARG <= 20) and fold(+, @, @ARG) == 2, each(@, if(@ < 20, @ += @ARG))))"
                        .parse::<Expr>()
                        .unwrap(),
                    when: WhenCondition::OnFeatureAdd,
                }]),
                prerequisites: None,
            },
        );
        let feat_index = FeaturesView::from_natural(&feat_index_map);

        let plan = plan_from_interleaving_with_caches(&original.core, feat_index, caches)
            .expect("rebuild must succeed when stored Generation + ASI bring CHA to 13");

        let class_names: Vec<&str> = plan
            .iter()
            .filter(|pending| {
                feat_index_map.get(&pending.name).is_some_and(|definition| {
                    matches!(
                        definition.category,
                        FeatureCategory::System(IdentitySlot::Class)
                    )
                })
            })
            .map(|pending| pending.name.as_ref())
            .collect();
        assert!(
            class_names.contains(&"Monk"),
            "plan must include Monk class steps, got {class_names:?}"
        );
        assert!(
            class_names.contains(&"Bard"),
            "plan must include Bard class step (prereq CHA=13 satisfied via Generation+ASI replay), got {class_names:?}"
        );
    }
}
