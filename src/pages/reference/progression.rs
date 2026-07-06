//! Read-only preview of per-class-level scaling for the `/r/class/:name`
//! reference page. Builds a `ProgressionPreview` by evaluating a feature's
//! `OnFeatureAdd` + `OnCompute` `assign` expressions for each class level
//! 1..=20 through `rules::PreviewContext` — no real `Character` is needed
//! because the table only shows numeric counts and slot totals, not
//! actual `Spell` objects.

use crate::{
    model::{AttrKey, Attribute, SpellData, format_bonus},
    rules::{
        FeatureDefinition, PoolKind, PoolSummary, PreviewContext, WhenCondition, eval_at_levels,
    },
};

/// Per-level value snapshots for each pool of a feature's assigns,
/// indexed `[level_index][pool_index]`. Level index is `level - 1`.
/// Levels before `feature_level` (where the class grants the feature)
/// render blank — the synthetic eval doesn't know grant levels itself.
pub fn preview_pool_values(
    feat_def: &FeatureDefinition,
    pools: &[PoolSummary],
    feature_level: u32,
) -> Vec<Vec<String>> {
    let mut rows = Vec::with_capacity(20);
    eval_at_levels(feat_def, |level, ctx| {
        let row = if level < feature_level {
            vec!["\u{2014}".to_string(); pools.len()]
        } else {
            pools
                .iter()
                .map(|pool| format_pool_value(ctx, pool.name, pool.kind))
                .collect()
        };
        rows.push(row);
    });
    rows
}

fn format_pool_value(ctx: &PreviewContext, name: &str, kind: PoolKind) -> String {
    let key = AttrKey::named(name);
    match kind {
        PoolKind::Points => {
            let max = ctx
                .values
                .get(&Attribute::PointsMax(key))
                .copied()
                .unwrap_or(0);
            if max > 0 {
                max.to_string()
            } else {
                "\u{2014}".into()
            }
        }
        PoolKind::Die => {
            let sides = ctx
                .values
                .get(&Attribute::DieSides(key))
                .copied()
                .unwrap_or(0);
            let count = ctx
                .values
                .get(&Attribute::DieCount(key))
                .copied()
                .unwrap_or(0);
            if count > 0 && sides > 0 {
                format!("{count}d{sides}")
            } else {
                "\u{2014}".into()
            }
        }
        PoolKind::Bonus => {
            let value = ctx.values.get(&Attribute::Bonus(key)).copied().unwrap_or(0);
            if value != 0 {
                format_bonus(value)
            } else {
                "\u{2014}".into()
            }
        }
        PoolKind::Choice => {
            let value = ctx
                .values
                .get(&Attribute::ChoiceCount(key))
                .copied()
                .unwrap_or(0);
            if value > 0 {
                value.to_string()
            } else {
                "\u{2014}".into()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressionRow {
    pub cantrips: u32,
    pub ready: u32,
    pub known: u32,
    pub slots: [u32; 9],
}

#[derive(Debug, Clone)]
pub struct ProgressionPreview {
    pub rows: Vec<ProgressionRow>,
    pub max_slot_level: u8,
    pub has_cantrips: bool,
    pub has_ready: bool,
    pub has_known: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::PoolSummarizer;

    #[test]
    fn pool_values_blank_before_the_feature_level() {
        // Constant pool (Mystic Arcanum shape) granted at class level 11.
        let feat_def: FeatureDefinition = serde_json::from_value(serde_json::json!({
            "name": "Mystic Arcanum (6th)",
            "assign": [
                {"expr": "POINTS.`Mystic Arcanum (6th)`.MAX = 1", "when": "OnCompute"},
            ],
        }))
        .expect("feature def");
        let pools = PoolSummarizer::new(&feat_def).pools();
        assert_eq!(pools.len(), 1);

        let rows = preview_pool_values(&feat_def, &pools, 11);
        assert!(
            rows[..10].iter().all(|row| row[0] == "\u{2014}"),
            "levels before the feature level stay blank: {rows:?}"
        );
        assert!(
            rows[10..].iter().all(|row| row[0] == "1"),
            "feature level onward shows the value: {rows:?}"
        );
    }
}

/// Per-class-level scaling preview (cantrips / slots / ready / known) for
/// a single caster feature, used by the `/r/class` reference page. Stays
/// inline rather than going through `eval_at_levels` because `CasterLevel`
/// has to be re-bound per level *before* `OnCompute` runs (the helper
/// only exposes a post-eval callback).
pub fn preview_progression(feat_def: &FeatureDefinition) -> ProgressionPreview {
    let mut ctx = PreviewContext::new();

    for assign in feat_def
        .assign
        .iter()
        .flatten()
        .filter(|assignment| assignment.when == WhenCondition::OnFeatureAdd)
    {
        if let Err(error) = assign.expr.apply(&mut ctx) {
            log::debug!(
                "preview_progression: OnFeatureAdd failed for '{}': {error:?}",
                feat_def.name
            );
        }
    }
    let coef = ctx
        .values
        .get(&Attribute::CasterCoef)
        .copied()
        .unwrap_or(0)
        .max(0) as u32;
    let pool = ctx.pool;

    let mut rows = Vec::with_capacity(20);
    for level in 1..=20u32 {
        ctx.values
            .insert(Attribute::ClassLevel(AttrKey::Scoped), level as i32);
        let caster_level = SpellData::single_caster_level(level, coef);
        ctx.values
            .insert(Attribute::CasterLevel(Some(pool)), caster_level as i32);

        for assign in feat_def
            .assign
            .iter()
            .flatten()
            .filter(|assignment| assignment.when == WhenCondition::OnCompute)
        {
            if let Err(error) = assign.expr.apply(&mut ctx) {
                log::debug!(
                    "preview_progression: OnCompute at L{level} failed for '{}': {error:?}",
                    feat_def.name
                );
            }
        }

        rows.push(ProgressionRow {
            cantrips: ctx.read_count(Attribute::SpellCantrips),
            ready: ctx.read_count(Attribute::SpellReady),
            known: ctx.read_count(Attribute::SpellKnown),
            slots: std::array::from_fn(|i| {
                ctx.read_count(Attribute::Slot(Some(pool), (i + 1) as u8))
            }),
        });
    }

    let max_slot_level = (1..=9u8)
        .rev()
        .find(|&n| rows.iter().any(|row| row.slots[(n - 1) as usize] > 0))
        .unwrap_or(0);
    let has_cantrips = rows.iter().any(|row| row.cantrips > 0);
    let has_ready = rows.iter().any(|row| row.ready > 0);
    let has_known = rows.iter().any(|row| row.known > 0);

    ProgressionPreview {
        rows,
        max_slot_level,
        has_cantrips,
        has_ready,
        has_known,
    }
}
