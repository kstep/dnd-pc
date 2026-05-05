use std::collections::BTreeMap;

use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::apply::{apply_with_prefilled_args, mark_all_applied},
    model::{Character, FeatureSource},
    rules::{
        RecomputePending, RulesRegistry,
        apply::{PendingFeature, collect_pending_features},
    },
};

/// Open the cascade modal seeded with a `Class Level` placeholder.
/// Single entry point for every level-up / multiclass-add affordance —
/// `+ Add / Level Up` in the Stats tab and the per-class dropdown in the
/// header both flow through here. `prefilled_class` short-circuits the
/// picker: when `Some(name)`, the chosen class lands as the replacement
/// without user interaction; when `None`, the picker shows all eligible
/// `System(Class)` candidates.
pub fn level_up_class(
    store: Store<Character>,
    registry: RulesRegistry,
    prefilled_class: Option<String>,
) {
    let target_level = store.read_untracked().level().saturating_add(1);
    let placeholder = PendingFeature {
        name: "Class Level".into(),
        source: FeatureSource::User(target_level),
        level: target_level,
    };

    let recompute_placeholder = placeholder.clone();
    let recompute: RecomputePending = Box::new(move |speculative: &Character| {
        registry.with_features_index_untracked(|features_index| {
            let class_summary: Vec<(String, u32)> = speculative
                .identity
                .classes
                .iter()
                .map(|class_level| (class_level.class.clone(), class_level.level))
                .collect();
            log::info!(
                "level_up recompute: speculative.classes = {class_summary:?}, total_level = {}",
                speculative.level()
            );

            let mut pending = vec![recompute_placeholder.clone()];
            let collected = collect_pending_features(speculative, &registry, features_index);
            log::info!(
                "level_up recompute: collect_pending_features returned {} entries: {:?}",
                collected.len(),
                collected
                    .iter()
                    .map(|pending_feat| &pending_feat.name)
                    .collect::<Vec<_>>()
            );
            pending.extend(collected);

            let result: Vec<_> = pending
                .into_iter()
                .filter_map(|pending_feat| {
                    let feat_def = features_index.get(pending_feat.name.as_str())?;
                    pending_feat.pending_inputs(feat_def, speculative)
                })
                .collect();
            log::info!(
                "level_up recompute: returning {} PendingInputs: {:?}",
                result.len(),
                result.iter().map(|pi| &pi.feature_name).collect::<Vec<_>>()
            );
            result
        })
    });

    let mut prefilled_replacements = BTreeMap::new();
    if let Some(class_name) = prefilled_class {
        prefilled_replacements.insert("Class Level".into(), class_name);
    }

    apply_with_prefilled_args(
        store,
        registry,
        vec![placeholder],
        BTreeMap::new(),
        prefilled_replacements,
        Some(recompute),
        mark_all_applied,
    );
}
