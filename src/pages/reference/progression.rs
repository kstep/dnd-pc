//! Read-only preview of per-class-level scaling for the `/r/class/:name`
//! reference page. Builds a `ProgressionPreview` by evaluating a feature's
//! `OnFeatureAdd` + `OnCompute` `assign` expressions for each class level
//! 1..=20 against an in-memory `BTreeMap<Attribute, i32>` — no real
//! `Character` is needed because the table only shows numeric counts and
//! slot totals, not actual `Spell` objects.

use std::collections::BTreeMap;

use crate::{
    expr,
    model::{Attribute, SpellSlotPool},
    rules::{FeatureDefinition, WhenCondition},
};

/// Numeric-only context: stores attribute writes in a flat map and
/// resolves reads against it. Implicit-pool `Slot`/`SlotUsed`/`CasterLevel`
/// are normalised through `self.pool` (mirroring the live `Context` impl)
/// so a single feature's expressions see consistent pool routing.
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

    fn normalize(&self, var: Attribute) -> Attribute {
        match var {
            Attribute::Slot(None, n) => Attribute::Slot(Some(self.pool), n),
            Attribute::SlotUsed(None, n) => Attribute::SlotUsed(Some(self.pool), n),
            Attribute::CasterLevel(None) => Attribute::CasterLevel(Some(self.pool)),
            other => other,
        }
    }
}

impl expr::Context<Attribute, i32> for PreviewContext {
    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        Ok(self.values.get(&self.normalize(var)).copied().unwrap_or(0))
    }

    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        if matches!(var, Attribute::SlotPool)
            && let Ok(pool) = SpellSlotPool::try_from(value.max(0) as u8)
        {
            self.pool = pool;
        }
        self.values.insert(self.normalize(var), value);
        Ok(())
    }
}

/// Single-class caster level: `ceil(class_level / coef)` with start
/// threshold (third casters at L3, half/full at L1). Mirrors
/// `Character::caster_info` single-class branch.
fn single_class_caster_level(class_level: u32, coef: u32) -> u32 {
    if coef == 0 {
        return 0;
    }
    let threshold = if coef == 3 { 3 } else { 1 };
    if class_level < threshold {
        0
    } else {
        class_level.div_ceil(coef)
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
    /// At least one level has a non-zero `SPELL.READY` (prepared / spells
    /// known to prepare). Gates the "Spells Ready" column.
    pub has_ready: bool,
    /// At least one level has a non-zero `SPELL.KNOWN` (spellbook size,
    /// two-tier casters only — Wizard, Sorcerer, Bard …). Gates the
    /// "Spells Known" column.
    pub has_known: bool,
}

/// Build a 20-row progression preview for `feat_def`. Phase 1 evaluates
/// `OnFeatureAdd` once to capture pool / ability / coef. Phase 2 sweeps
/// class levels 1..=20, pre-seeds `CLASS_LEVEL` and `CASTER_LEVEL.{pool}`,
/// runs `OnCompute`, and snapshots the resulting counts.
pub fn preview_progression(feat_def: &FeatureDefinition) -> ProgressionPreview {
    let mut ctx = PreviewContext::new();

    // Phase 1: OnFeatureAdd — writes SLOT.POOL, CASTER_ABILITY, CASTER_COEF.
    for assign in feat_def
        .assign
        .iter()
        .flatten()
        .filter(|assignment| assignment.when == WhenCondition::OnFeatureAdd)
    {
        let _ = assign.expr.apply(&mut ctx);
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
        let caster_level = single_class_caster_level(level, coef);
        ctx.values
            .insert(Attribute::CasterLevel(Some(pool)), caster_level as i32);

        // Phase 2: OnCompute — writes SLOT.N + SPELL.{CANTRIPS,READY,KNOWN}.
        for assign in feat_def
            .assign
            .iter()
            .flatten()
            .filter(|assignment| assignment.when == WhenCondition::OnCompute)
        {
            let _ = assign.expr.apply(&mut ctx);
        }

        rows.push(ProgressionRow {
            cantrips: read_count(&ctx, Attribute::SpellCantrips),
            ready: read_count(&ctx, Attribute::SpellReady),
            known: read_count(&ctx, Attribute::SpellKnown),
            slots: std::array::from_fn(|i| {
                read_count(&ctx, Attribute::Slot(Some(pool), (i + 1) as u8))
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

fn read_count(ctx: &PreviewContext, var: Attribute) -> u32 {
    ctx.values
        .get(&ctx.normalize(var))
        .copied()
        .unwrap_or(0)
        .max(0) as u32
}
