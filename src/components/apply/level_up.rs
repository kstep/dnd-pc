use leptos::prelude::*;
use reactive_stores::Store;

use super::modal_flow::apply_with_modal;
use crate::{
    model::Character,
    rules::{
        DefinitionStore, RulesRegistry,
        apply::{
            apply_new_features, collect_class_features, collect_pending_features, reapply_existing,
        },
    },
};

/// Collect pending features and apply them via modal.
pub fn apply_level(store: Store<Character>, registry: RulesRegistry) {
    let pending = store.with_untracked(|character| {
        registry
            .with_features_index_untracked(|fi| collect_pending_features(character, &registry, fi))
    });

    apply_with_modal(
        store,
        registry,
        pending,
        None,
        move |character, pending, inputs, fi| {
            // Mark species/background as applied if they had pending features
            if !character.applied.species && !character.identity.species.is_empty() {
                character.applied.species = true;
            }
            if !character.applied.background && !character.identity.background.is_empty() {
                character.applied.background = true;
            }
            // Mark class levels as applied
            let class_cache = registry.classes().cache().read_untracked();
            let class_updates: Vec<(String, u32)> = character
                .identity
                .classes
                .iter()
                .map(|cl| (cl.class.clone(), cl.level))
                .collect();
            for (class_name, level) in &class_updates {
                for lvl in 1..=*level {
                    character.applied.mark_level(class_name, lvl);
                }
            }
            for class_level in &mut character.identity.classes {
                if let Some(def) = class_cache.get(class_level.class.as_str()) {
                    class_level.hit_die_sides = def.hit_die;
                }
            }

            reapply_existing(fi, character);
            apply_new_features(fi, character, pending, Some(&inputs.feature_inputs));
            character.combat.hp_current = character.hp_max();

            let xp_threshold = character.xp_threshold();
            if character.identity.experience_points < xp_threshold {
                character.identity.experience_points = xp_threshold;
            }
        },
    );
}

/// Apply a single class level only (used by per-level apply buttons).
pub fn apply_single_level(
    store: Store<Character>,
    registry: RulesRegistry,
    class_index: usize,
    level: u32,
) {
    let pending = store.with_untracked(|character| {
        let class_cache = registry.classes().cache().read_untracked();
        registry.with_features_index_untracked(|fi| {
            class_cache
                .get(character.identity.classes[class_index].class.as_str())
                .into_iter()
                .flat_map(|class_def| {
                    collect_class_features(character, class_index, level, class_def, fi)
                })
                .collect()
        })
    });

    apply_with_modal(
        store,
        registry,
        pending,
        None,
        move |character, pending, inputs, fi| {
            if let Some(class_level) = character.identity.classes.get_mut(class_index) {
                registry.classes().with(&class_level.class, |def| {
                    class_level.hit_die_sides = def.hit_die;
                });
                let class_name = class_level.class.clone();
                character.applied.mark_level(&class_name, level);
            }
            reapply_existing(fi, character);
            apply_new_features(fi, character, pending, Some(&inputs.feature_inputs));
            character.combat.hp_current = character.hp_max();
        },
    );
}
