use std::collections::BTreeMap;

use crate::{
    model::{Character, CharacterIdentity, Context, Expr, FeatureSource, FeatureValue},
    rules::{WhenCondition, feature::FeatureDefinition, spells::SpellDefinition},
};

/// Recompute derived character state. Call after any apply pipeline step
/// that mutates `character.features` so callers can trust the result is
/// finalized.
pub fn compute(
    character: &mut Character,
    feat_index: &BTreeMap<Box<str>, FeatureDefinition>,
    spell_index: &BTreeMap<Box<str>, SpellDefinition>,
) {
    character.compute();
    refresh_spell_structure(character, feat_index, spell_index);
    assign(character, feat_index, WhenCondition::OnCompute);
    character.compute_armor_class();
    recompute_dynamic_fields(character, feat_index);
}

/// Evaluate assignment expressions across all features for the given
/// condition. CLASS_LEVEL is taken from the current level of the class
/// named in `feature.source` — no class-cache lookup needed.
pub fn assign(
    character: &mut Character,
    feat_index: &BTreeMap<Box<str>, FeatureDefinition>,
    when: WhenCondition,
) {
    // Two-phase: collect borrowed assignment refs while iter holds &character,
    // then mutate &mut character in the apply phase. Using &Expr (Arc-backed,
    // cheap clone if ever needed) and Option<&str> for scope avoids the
    // String::from / as_deref round-trip.
    type ScopedExprs<'a> = BTreeMap<Option<&'a str>, Vec<&'a Expr>>;
    let feature_entries: Vec<(String, ScopedExprs<'_>, i32)> = character
        .features
        .iter()
        .filter_map(|feat| {
            let feat_def = feat_index.get(feat.name.as_str())?;
            let mut by_scope: ScopedExprs<'_> = BTreeMap::new();
            for assignment in feat_def
                .assign
                .iter()
                .flatten()
                .filter(|assignment| assignment.when == when)
            {
                by_scope
                    .entry(assignment.scope.as_deref())
                    .or_default()
                    .push(&assignment.expr);
            }
            if by_scope.is_empty() {
                return None;
            }
            let class_level = class_level_for_source(&character.identity, &feat.source) as i32;
            Some((feat.name.clone(), by_scope, class_level))
        })
        .collect();

    for (feat_name, by_scope, class_level) in &feature_entries {
        for (scope, exprs) in by_scope {
            let target = scope.unwrap_or(feat_name.as_str());
            let points = character
                .features
                .get(target)
                .map(Context::extract_points)
                .unwrap_or_default();

            let mut ctx = Context {
                character,
                class_level: *class_level,
                feature: Some(target.to_string()),
                points,
            };
            for expr in exprs {
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
fn refresh_spell_structure(
    character: &mut Character,
    feat_index: &BTreeMap<Box<str>, FeatureDefinition>,
    spell_index: &BTreeMap<Box<str>, SpellDefinition>,
) {
    // Two-phase: collect &FeatureDefinition + level while only reading
    // character (iter holds &features), then apply mutates &mut character.
    let updates: Vec<(&FeatureDefinition, u32)> = character
        .features
        .iter()
        .filter_map(|feature| {
            let feat_def = feat_index.get(feature.name.as_str())?;
            feat_def.spells.as_ref()?;
            let level = character.effective_level_for(&feature.source);
            Some((feat_def, level))
        })
        .collect();
    for (feat_def, level) in updates {
        if let Some(spells_def) = &feat_def.spells {
            spells_def.apply(feat_def, level, character, spell_index);
        }
    }
}

/// Re-evaluate dynamic field values (Points max, Die amount) after
/// ability scores or other stats may have changed. Iterates feature list
/// (not data map) so `feature.source` drives class-level lookup.
fn recompute_dynamic_fields(
    character: &mut Character,
    feat_index: &BTreeMap<Box<str>, FeatureDefinition>,
) {
    let mut updates: Vec<(String, usize, FeatureValue)> = Vec::new();
    for feature in character.features.iter() {
        let Some(feat_def) = feat_index.get(feature.name.as_str()) else {
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
