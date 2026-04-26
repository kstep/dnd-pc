use std::collections::BTreeMap;

use crate::{
    model::{Character, CharacterIdentity, Context, Expr, FeatureSource, FeatureValue},
    rules::{WhenCondition, feature::FeatureDefinition},
};

/// Recompute derived character state. Call after any apply pipeline step
/// that mutates `character.features` so callers can trust the result is
/// finalized.
pub fn compute(character: &mut Character, fi: &BTreeMap<Box<str>, FeatureDefinition>) {
    character.compute();
    refresh_spell_structure(character, fi);
    assign(character, fi, WhenCondition::OnCompute);
    character.compute_armor_class();
    recompute_dynamic_fields(character, fi);
}

/// Evaluate assignment expressions across all features for the given
/// condition. CLASS_LEVEL is taken from the current level of the class
/// named in `feature.source` — no class-cache lookup needed.
pub fn assign(
    character: &mut Character,
    fi: &BTreeMap<Box<str>, FeatureDefinition>,
    when: WhenCondition,
) {
    let feature_entries: Vec<_> = character
        .features
        .iter()
        .filter_map(|feat| {
            let feat_def = fi.get(feat.name.as_str())?;
            let assignments: Vec<_> = feat_def
                .assign
                .iter()
                .flat_map(|assigns| assigns.iter())
                .filter(|assignment| assignment.when == when)
                .collect();
            if assignments.is_empty() {
                return None;
            }

            let class_level = class_level_for_source(&character.identity, &feat.source);
            let mut scope_groups: Vec<(Option<&str>, Vec<Expr>)> = Vec::new();
            for assignment in &assignments {
                let scope = assignment.scope.as_deref();
                if let Some(group) = scope_groups.iter_mut().find(|(s, _)| *s == scope) {
                    group.1.push(assignment.expr.clone());
                } else {
                    scope_groups.push((scope, vec![assignment.expr.clone()]));
                }
            }

            Some((
                feat.name.clone(),
                scope_groups
                    .into_iter()
                    .map(|(scope, exprs)| (scope.map(String::from), exprs))
                    .collect::<Vec<_>>(),
                class_level as i32,
            ))
        })
        .collect();

    for (feat_name, scope_groups, class_level) in feature_entries {
        for (scope, exprs) in scope_groups {
            let target = scope.as_deref().unwrap_or(&feat_name);
            let points = character
                .features
                .get(target)
                .map(Context::extract_points)
                .unwrap_or_default();

            let mut ctx = Context {
                character,
                class_level,
                feature: Some(target.to_string()),
                points,
            };
            for expr in &exprs {
                if let Err(error) = expr.apply(&mut ctx) {
                    log::error!("Failed to apply assignment: {error:?}");
                }
            }

            if let Some(feature_data) = ctx.character.features.get_mut(target) {
                Context::writeback_points(feature_data, &ctx.points);
            }
        }
    }
}

/// Per-feature SpellData bootstrap: skeleton + sticky import + free_uses.
fn refresh_spell_structure(character: &mut Character, fi: &BTreeMap<Box<str>, FeatureDefinition>) {
    let updates: Vec<(String, u32, u32)> = character
        .features
        .iter()
        .filter_map(|feature| {
            let feat_def = fi.get(feature.name.as_str())?;
            feat_def.spells.as_ref()?;
            let level = character.effective_level_for(&feature.source);
            let free_uses_max = feat_def.free_uses_max(level, character);
            Some((feature.name.clone(), level, free_uses_max))
        })
        .collect();
    for (feat_name, level, free_uses_max) in updates {
        if let Some(feat_def) = fi.get(feat_name.as_str())
            && let Some(spells_def) = &feat_def.spells
        {
            spells_def.apply(level, character, &feat_name, free_uses_max);
        }
    }
}

/// Re-evaluate dynamic field values (Points max, Die amount) after
/// ability scores or other stats may have changed. Iterates feature list
/// (not data map) so `feature.source` drives class-level lookup.
fn recompute_dynamic_fields(character: &mut Character, fi: &BTreeMap<Box<str>, FeatureDefinition>) {
    let mut updates: Vec<(String, usize, FeatureValue)> = Vec::new();
    for feature in character.features.iter() {
        let Some(feat_def) = fi.get(feature.name.as_str()) else {
            continue;
        };
        let class_level = class_level_for_source(&character.identity, &feature.source);
        let Some(entry) = character.features.get(&feature.name) else {
            continue;
        };
        for (i, field) in entry.fields.iter().enumerate() {
            let Some(field_def) = feat_def.fields.get(field.name.as_str()) else {
                continue;
            };
            if let Some(new_val) = field_def.kind.recompute_dynamic(class_level, character) {
                updates.push((feature.name.clone(), i, new_val));
            }
        }
    }

    for (feat_name, field_idx, new_val) in updates {
        if let Some(entry) = character.features.get_mut(&feat_name)
            && let Some(field) = entry.fields.get_mut(field_idx)
        {
            match (&new_val, &mut field.value) {
                (FeatureValue::Points { max: new_max, .. }, FeatureValue::Points { max, .. }) => {
                    *max = *new_max;
                }
                (FeatureValue::Die { die: new_die, .. }, FeatureValue::Die { die, .. }) => {
                    *die = *new_die;
                }
                _ => {}
            }
        }
    }
}

/// Current level of the class named in `source`, looked up via
/// `identity.classes`. Source carries the class name; the level inside
/// the source variant is the *granted* level, not the current one.
fn class_level_for_source(identity: &CharacterIdentity, source: &FeatureSource) -> u32 {
    let class_name: &str = match source {
        FeatureSource::Class(name, _) | FeatureSource::Subclass(name, _, _) => name,
        _ => return 0,
    };
    identity
        .classes
        .iter()
        .find(|cl| cl.class == class_name)
        .map(|cl| cl.level)
        .unwrap_or(0)
}
