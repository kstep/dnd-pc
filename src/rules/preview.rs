//! Synthetic eval context for previewing assignment expressions without a
//! real `Character`. Used by reference pages and `PoolSummarizer` to walk
//! per-level snapshots of feature pools and spell grants.

use std::collections::BTreeMap;

use crate::{
    expr,
    model::{AttrKey, Attribute, AttributeGroup, SpellSlotPool},
    rules::{WhenCondition, feature::FeatureDefinition},
};

/// Stand-in `Context` that stores assigns into a flat `BTreeMap` and
/// resolves through it. No real character is touched. The active pool
/// (`SpellSlotPool`) tracks the most recent `SLOT.POOL = N` write and is
/// used to substitute `Slot(None, _)` / `SlotUsed(None, _)` /
/// `CasterLevel(None)` lookups with the current scoped pool.
pub struct PreviewContext {
    pub values: BTreeMap<Attribute, i32>,
    pub pool: SpellSlotPool,
}

impl PreviewContext {
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
            pool: SpellSlotPool::Arcane,
        }
    }

    /// Read the resolved value of `var` clamped to non-negative `u32`.
    pub fn read_count(&self, var: Attribute) -> u32 {
        use expr::Context as _;
        self.resolve(var).unwrap_or(0).max(0) as u32
    }
}

impl Default for PreviewContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-bind `OnFeatureAdd` assigns into a fresh `PreviewContext`, then
/// iterate `level: 1..=20` running `OnCompute` assigns each iteration and
/// invoking `per_level` with the post-eval ctx. Returns the final `ctx`
/// for callers that want to read terminal state (`coef`, etc.). Errors
/// during eval are logged at debug level — caller may still observe via
/// `ctx.values` snapshot.
pub fn eval_at_levels<F: FnMut(u32, &mut PreviewContext)>(
    feat_def: &FeatureDefinition,
    mut per_level: F,
) -> PreviewContext {
    let mut ctx = PreviewContext::new();
    for assign in feat_def.assign.iter().flatten() {
        if assign.when == WhenCondition::OnFeatureAdd
            && let Err(error) = assign.expr.apply(&mut ctx)
        {
            log::debug!(
                "eval_at_levels: OnFeatureAdd failed for '{}': {error:?}",
                feat_def.name
            );
        }
    }
    for level in 1..=20u32 {
        ctx.values.insert(Attribute::Level, level as i32);
        ctx.values
            .insert(Attribute::ClassLevel(AttrKey::Scoped), level as i32);
        for assign in feat_def.assign.iter().flatten() {
            if assign.when == WhenCondition::OnCompute
                && let Err(error) = assign.expr.apply(&mut ctx)
            {
                log::debug!(
                    "eval_at_levels: OnCompute at L{level} failed for '{}': {error:?}",
                    feat_def.name
                );
            }
        }
        per_level(level, &mut ctx);
    }
    ctx
}

// PreviewContext has no Character — group iteration yields nothing. Reference
// pages only preview scalar progressions; loops over @ABIL/@SKILL/etc.
// surface zero contributions for the snapshot.
impl expr::ResolveGroup<AttributeGroup> for PreviewContext {
    fn resolve_group<'a>(
        &'a self,
        _grp: &AttributeGroup,
    ) -> Box<dyn Iterator<Item = Vec<Attribute>> + 'a> {
        Box::new(std::iter::empty())
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
