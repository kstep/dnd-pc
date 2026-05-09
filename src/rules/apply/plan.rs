use std::collections::BTreeMap;

use crate::{
    model::{
        Character, CharacterIdentity, Feature, FeatureCategory, FeatureSource, Features,
        IdentitySlot,
    },
    rules::{
        ClassIndexEntry, FeaturesView, RulesRegistry,
        apply::{
            DefinitionCaches,
            collect::{class_level_sources, collect_background_features, collect_species_features},
            pending::{PICK_BACKGROUND, PICK_SPECIES, PendingFeature},
            primitives::apply_pending,
            rebuild::{DefinitionKind, RebuildError},
        },
    },
};

/// Build the canonical level-up plan from `identity` + `features`. Prefers
/// the marker path; falls back to interleaving for legacy chars without
/// markers.
pub fn level_up_plan(
    identity: &CharacterIdentity,
    features: &Features,
    registry: &RulesRegistry,
) -> Result<Vec<PendingFeature>, RebuildError> {
    if let Some(plan) = plan_from_markers(identity, features) {
        return Ok(plan);
    }
    plan_from_interleaving(identity, features, registry)
}

/// Build a plan straight from the System(_) markers in `features.list`.
/// Returns `None` when markers don't cover every level implied by
/// `identity.classes` — caller falls back to interleaving.
pub fn plan_from_markers(
    identity: &CharacterIdentity,
    features: &Features,
) -> Option<Vec<PendingFeature>> {
    // Group System(Class) markers by class name and check coverage.
    let mut class_markers: BTreeMap<String, Vec<&Feature>> = BTreeMap::new();
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
    let mut class_running: BTreeMap<String, u32> = BTreeMap::new();
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
        });

        // Subclass marker for this class+level, if present.
        let subclass_match = features.iter().find(|feature| {
            matches!(
                feature.category,
                FeatureCategory::System(IdentitySlot::Subclass)
            ) && matches!(
                &feature.source,
                FeatureSource::Class(class_name, level)
                    if class_name.as_ref() == class_marker.name.as_str()
                        && *level == class_level
            )
        });
        if let Some(subclass) = subclass_match {
            plan.push(PendingFeature {
                name: subclass.name.clone(),
                source: subclass.source.clone(),
                level: class_level,
            });
        }

        emit_user_features(features, &mut plan, character_level);
    }

    Some(plan)
}

/// Append a System(slot) marker for `slot` (Species or Background) to `plan`.
/// Prefers an existing marker in `features.list`; falls back to synthesizing
/// from `identity` for legacy half-state. When neither marker nor identity
/// name is set, emits the `Generation: Species` / `Generation: Background`
/// placeholder (`replace_with: Category(System(_))`) so the args modal can
/// surface a picker during cascade. Other slots are no-op.
fn push_identity_marker(
    identity: &CharacterIdentity,
    features: &Features,
    plan: &mut Vec<PendingFeature>,
    slot: IdentitySlot,
) {
    if let Some(marker) = find_system_marker(features, slot) {
        plan.push(PendingFeature {
            name: marker.name.clone(),
            source: marker.source.clone(),
            level: 0,
        });
        return;
    }
    let (identity_name, placeholder) = match slot {
        IdentitySlot::Species => (&identity.species, PICK_SPECIES),
        IdentitySlot::Background => (&identity.background, PICK_BACKGROUND),
        IdentitySlot::Class | IdentitySlot::Subclass => return,
    };
    let name = if identity_name.is_empty() {
        placeholder.to_string()
    } else {
        identity_name.clone()
    };
    plan.push(PendingFeature {
        name,
        source: FeatureSource::User(0),
        level: 0,
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
    identity: &CharacterIdentity,
    features: &Features,
    registry: &RulesRegistry,
) -> Result<Vec<PendingFeature>, RebuildError> {
    registry.with_definitions(|caches| {
        registry.with_features_index_untracked(|feat_index| {
            registry.with_class_entries(|class_entries| {
                plan_from_interleaving_with_caches(
                    identity,
                    features,
                    feat_index,
                    caches,
                    class_entries,
                )
            })
        })
    })
}

/// Caches-explicit form of [`plan_from_interleaving`]. Kept as a separate
/// function so unit tests can supply hand-built caches without spinning up a
/// real [`RulesRegistry`].
pub fn plan_from_interleaving_with_caches(
    identity: &CharacterIdentity,
    features: &Features,
    feat_index: FeaturesView<'_>,
    caches: DefinitionCaches,
    class_entries: &BTreeMap<Box<str>, ClassIndexEntry>,
) -> Result<Vec<PendingFeature>, RebuildError> {
    let mut plan: Vec<PendingFeature> = Vec::new();

    // 1. User(0) non-System feats (e.g. Generation: *).
    emit_user_features(features, &mut plan, 0);

    // Throwaway character used solely to evaluate multiclass prereqs against
    // a realistic post-species/background ability state.
    let mut probe = Character::from_identity(identity.clone());
    // Reset identity-derived state so re-application via System markers
    // doesn't double-stack (CLASS.LEVEL += 1 reads the current level).
    for class_level in probe.identity.classes.iter_mut() {
        class_level.level = 0;
    }

    // 2. System(Species) marker + features simulated on probe (so subsequent
    //    multiclass prereq checks see post-species ability state).
    let species_marker_idx = plan.len();
    push_identity_marker(identity, features, &mut plan, IdentitySlot::Species);
    if let Some(marker) = plan.get(species_marker_idx).cloned()
        && !identity.species.is_empty()
    {
        let species_def = caches
            .species
            .get(identity.species.as_str())
            .ok_or_else(|| RebuildError::MissingDefinition {
                kind: DefinitionKind::Species,
                name: identity.species.clone(),
            })?;
        apply_pending(feat_index, &mut probe, &marker, &[], caches, false);
        let species_pending: Vec<PendingFeature> =
            collect_species_features(&probe, species_def, feat_index).collect();
        for pending in &species_pending {
            apply_pending(feat_index, &mut probe, pending, &[], caches, false);
        }
    }

    // 3. System(Background) marker — symmetric.
    let bg_marker_idx = plan.len();
    push_identity_marker(identity, features, &mut plan, IdentitySlot::Background);
    if let Some(marker) = plan.get(bg_marker_idx).cloned()
        && !identity.background.is_empty()
    {
        let bg_def = caches
            .backgrounds
            .get(identity.background.as_str())
            .ok_or_else(|| RebuildError::MissingDefinition {
                kind: DefinitionKind::Background,
                name: identity.background.clone(),
            })?;
        apply_pending(feat_index, &mut probe, &marker, &[], caches, false);
        let bg_pending: Vec<PendingFeature> =
            collect_background_features(&probe, bg_def, feat_index).collect();
        for pending in &bg_pending {
            apply_pending(feat_index, &mut probe, pending, &[], caches, false);
        }
    }

    // 4. Class levels — interleaved with multiclass prereq re-evaluation.
    apply_classes_interleaved(
        identity,
        features,
        feat_index,
        caches,
        class_entries,
        &mut probe,
        &mut plan,
    )?;

    Ok(plan)
}

/// Walk class levels in canonical order (primary first, multiclasses gated by
/// prereq each iteration), appending System markers + User(N) feats to `plan`.
/// `probe` is the throwaway character used for prereq checks; it ends in an
/// implementation-detail state and must be discarded.
fn apply_classes_interleaved(
    identity: &CharacterIdentity,
    features: &Features,
    feat_index: FeaturesView<'_>,
    caches: DefinitionCaches,
    class_entries: &BTreeMap<Box<str>, ClassIndexEntry>,
    probe: &mut Character,
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
        emit_class_level(identity, feat_index, caches, probe, plan, 0, 1)?;
        applied[0] = 1;
        character_level = 1;
        emit_user_features(features, plan, character_level);
    }

    while let Some(idx) = pick_next_class(probe, &targets, &applied, class_entries) {
        let next_class_lvl = applied[idx] + 1;
        emit_class_level(
            identity,
            feat_index,
            caches,
            probe,
            plan,
            idx,
            next_class_lvl,
        )?;
        applied[idx] = next_class_lvl;
        character_level += 1;
        emit_user_features(features, plan, character_level);
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
    probe: &Character,
    targets: &[u32],
    applied: &[u32],
    entries: &BTreeMap<Box<str>, ClassIndexEntry>,
) -> Option<usize> {
    let meets_prereq = |class_name: &str| {
        entries
            .get(class_name)
            .is_none_or(|entry| entry.meets_prerequisites(probe))
    };

    let idx = if meets_prereq(probe.identity.classes[0].class.as_str()) {
        probe
            .identity
            .classes
            .iter()
            .enumerate()
            .skip(1)
            .find(|(idx, class_level)| {
                !class_level.class.is_empty()
                    && applied[*idx] < targets[*idx]
                    && meets_prereq(class_level.class.as_str())
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
fn emit_class_level(
    identity: &CharacterIdentity,
    feat_index: FeaturesView<'_>,
    caches: DefinitionCaches,
    probe: &mut Character,
    plan: &mut Vec<PendingFeature>,
    class_idx: usize,
    class_level: u32,
) -> Result<(), RebuildError> {
    let class_name = identity.classes[class_idx].class.as_str();
    let class_def =
        caches
            .classes
            .get(class_name)
            .ok_or_else(|| RebuildError::MissingDefinition {
                kind: DefinitionKind::Class,
                name: class_name.to_owned(),
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

    plan.push(PendingFeature {
        name: class_name.to_owned(),
        source: FeatureSource::User(character_level),
        level: character_level,
    });
    apply_pending(
        feat_index,
        probe,
        plan.last().expect("just pushed"),
        &[],
        caches,
        false,
    );

    // Subclass marker fires at the lowest level the subclass declares
    // features for (matches `class_level_sources`'s subclass surfacing).
    if let Some(subclass_name) = identity.classes[class_idx].subclass.as_deref()
        && let Some(subclass_def) = class_def.subclasses.get(subclass_name)
    {
        let pick_level = subclass_def.levels.keys().next().copied().unwrap_or(0);
        if pick_level == class_level {
            plan.push(PendingFeature {
                name: subclass_name.to_owned(),
                source: FeatureSource::Class(Box::<str>::from(class_name), class_level),
                level: class_level,
            });
            apply_pending(
                feat_index,
                probe,
                plan.last().expect("just pushed"),
                &[],
                caches,
                false,
            );
        }
    }

    // Apply the class's level features on the probe so prereqs that read
    // post-ASI abilities see the realistic state. Use class_level_sources
    // directly (cheaper than collect_class_features which checks
    // already-applied; probe is fresh so dedup is a no-op).
    let class_level_obj = &identity.classes[class_idx];
    for (name, source) in class_level_sources(class_level_obj, class_level, class_def) {
        let pending = PendingFeature {
            name: name.to_owned(),
            source,
            level: class_level,
        };
        apply_pending(feat_index, probe, &pending, &[], caches, false);
    }

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
        });
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::*;
    use crate::model::{ClassLevel, Feature};

    fn marker(name: &str, slot: IdentitySlot, source: FeatureSource) -> Feature {
        Feature {
            name: name.to_string(),
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

        let plan = plan_from_markers(&original.identity, &original.features).expect("plan");
        let class_entries: Vec<(&str, &FeatureSource)> = plan
            .iter()
            .filter(|pending| pending.name == "Fighter" || pending.name == "Wizard")
            .map(|pending| (pending.name.as_str(), &pending.source))
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

        let plan = plan_from_markers(&original.identity, &original.features).expect("plan");
        // Find position of Fighter@User(3) and assert Champion sits next.
        let l3_pos = plan
            .iter()
            .position(|pending| {
                pending.name == "Fighter" && pending.source == FeatureSource::User(3)
            })
            .expect("L3 marker");
        let next = plan.get(l3_pos + 1).expect("after L3");
        assert_eq!(next.name, "Champion");
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

        let plan = plan_from_markers(&original.identity, &original.features).expect("plan");
        let class_steps = plan
            .iter()
            .filter(|pending| pending.name == "Fighter" && pending.source.is_user())
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

        assert!(plan_from_markers(&original.identity, &original.features).is_none());
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

        let plan = plan_from_markers(&original.identity, &original.features).expect("plan");
        let names: Vec<&str> = plan.iter().map(|pending| pending.name.as_str()).collect();
        assert!(
            names.contains(&PICK_SPECIES),
            "expected {PICK_SPECIES} placeholder for empty species, got {names:?}"
        );
        assert!(
            names.contains(&PICK_BACKGROUND),
            "expected {PICK_BACKGROUND} placeholder for empty background, got {names:?}"
        );
    }
}
