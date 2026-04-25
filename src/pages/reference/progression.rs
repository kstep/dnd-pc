//! Read-only preview of per-class-level scaling for the `/r/class/:name`
//! reference page. Builds a `ProgressionPreview` by evaluating a feature's
//! `OnFeatureAdd` + `OnCompute` `assign` expressions for each class level
//! 1..=20 against an in-memory `BTreeMap<Attribute, i32>` — no real
//! `Character` is needed because the table only shows numeric counts and
//! slot totals, not actual `Spell` objects.

use std::collections::BTreeMap;

use crate::{
    expr::{self, Context as _},
    model::{Attribute, SpellData, SpellSlotPool},
    rules::{FeatureDefinition, WhenCondition},
};

/// Stand-in for `Character`-backed `Context` when previewing scaling
/// expressions for a single feature (no real character involved).
pub struct PreviewContext {
    values: BTreeMap<Attribute, i32>,
    pool: SpellSlotPool,
}

impl PreviewContext {
    fn new() -> Self {
        Self {
            values: BTreeMap::new(),
            pool: SpellSlotPool::Arcane,
        }
    }

    fn read_count(&self, var: Attribute) -> u32 {
        self.resolve(var).unwrap_or(0).max(0) as u32
    }
}

impl expr::Context<Attribute, i32> for PreviewContext {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        let key = match var {
            Attribute::Slot(None, n) => Attribute::Slot(Some(self.pool), n),
            Attribute::SlotUsed(None, n) => Attribute::SlotUsed(Some(self.pool), n),
            Attribute::CasterLevel(None) => Attribute::CasterLevel(Some(self.pool)),
            other => other,
        };
        Ok(self.values.get(&key).copied().unwrap_or(0))
    }

    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        if matches!(var, Attribute::SlotPool)
            && let Ok(pool) = SpellSlotPool::try_from(value.max(0) as u8)
        {
            self.pool = pool;
        }
        let key = match var {
            Attribute::Slot(None, n) => Attribute::Slot(Some(self.pool), n),
            Attribute::SlotUsed(None, n) => Attribute::SlotUsed(Some(self.pool), n),
            Attribute::CasterLevel(None) => Attribute::CasterLevel(Some(self.pool)),
            other => other,
        };
        self.values.insert(key, value);
        Ok(())
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

/// Per-class-level scaling preview (cantrips / slots / ready / known) for
/// a single caster feature, used by the `/r/class` reference page.
pub fn preview_progression(feat_def: &FeatureDefinition) -> ProgressionPreview {
    let mut ctx = PreviewContext::new();

    for assign in feat_def
        .assign
        .iter()
        .flatten()
        .filter(|assignment| assignment.when == WhenCondition::OnFeatureAdd)
    {
        if let Err(error) = assign.expr.apply(&mut ctx) {
            log::warn!(
                "preview_progression: OnFeatureAdd expr failed for {}: {error:?}",
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
        ctx.values.insert(Attribute::ClassLevel, level as i32);
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
                log::warn!(
                    "preview_progression: OnCompute expr at L{level} failed for {}: {error:?}",
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
