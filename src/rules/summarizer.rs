//! Discover named pools (Points/Die/Bonus/Choice) and sticky-spell grants
//! from a FeatureDefinition's assign expressions. Used by reference pages
//! and tooltips.

use std::{
    collections::{BTreeMap, HashSet},
    hash::Hash,
};

use crate::{
    expr::{Context as _, Op},
    model::{AttrKey, Attribute},
    rules::{eval_at_levels, feature::FeatureDefinition},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoolKind {
    Points,
    Die,
    Bonus,
    Choice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PoolSummary {
    pub name: &'static str,
    pub kind: PoolKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StickyGrant {
    pub name: &'static str,
    pub min_level: u32,
}

pub struct PoolSummarizer<'a> {
    feat_def: &'a FeatureDefinition,
}

impl<'a> PoolSummarizer<'a> {
    pub fn new(feat_def: &'a FeatureDefinition) -> Self {
        Self { feat_def }
    }

    /// Walk all `Op::AssignVar` targets across every assignment, applying
    /// `classify` and collecting first occurrences (deduped via `HashSet`).
    fn collect_unique<T: Eq + Hash + Copy>(
        &self,
        classify: impl Fn(&Attribute) -> Option<T>,
    ) -> Vec<T> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        for assign in self.feat_def.assign.iter().flatten() {
            for op in assign.expr.ops() {
                if let Op::AssignVar(var) = op
                    && let Some(item) = classify(var)
                    && seen.insert(item)
                {
                    found.push(item);
                }
            }
        }
        found
    }

    /// Discover pools by walking feat_def.assign for AssignVar targets.
    /// Pool ordering matches first-write-encountered across the assign list.
    pub fn pools(&self) -> Vec<PoolSummary> {
        self.collect_unique(|var| {
            let (name, kind) = match var {
                Attribute::Points(AttrKey::Named(name))
                | Attribute::PointsMax(AttrKey::Named(name)) => (*name, PoolKind::Points),
                Attribute::DieSides(AttrKey::Named(name))
                | Attribute::DieCount(AttrKey::Named(name))
                | Attribute::DieUsed(AttrKey::Named(name)) => (*name, PoolKind::Die),
                Attribute::Bonus(AttrKey::Named(name)) => (*name, PoolKind::Bonus),
                Attribute::ChoiceCount(AttrKey::Named(name)) => (*name, PoolKind::Choice),
                _ => return None,
            };
            Some(PoolSummary { name, kind })
        })
    }

    /// Discover sticky spell grants. Each entry's `min_level` is the lowest
    /// `LEVEL`/`CLASS.LEVEL` at which `STICKY.<name>` first becomes 1 when
    /// the feature's `OnCompute` assigns are evaluated; level guards
    /// (`if(LEVEL >= N, ...)`) are reverse-engineered through eval rather
    /// than parsed from the `Op` slice.
    pub fn sticky_grants(&self) -> Vec<StickyGrant> {
        let names = self.collect_unique(|var| match var {
            Attribute::Sticky(AttrKey::Named(name)) => Some(*name),
            _ => None,
        });
        if names.is_empty() {
            return Vec::new();
        }

        let mut min_levels: BTreeMap<&'static str, u32> = BTreeMap::new();
        eval_at_levels(self.feat_def, |level, ctx| {
            for &name in &names {
                if min_levels.contains_key(name) {
                    continue;
                }
                let key = Attribute::Sticky(AttrKey::Named(name));
                if ctx.resolve(key).unwrap_or(0) == 1 {
                    min_levels.insert(name, level);
                }
            }
        });

        names
            .into_iter()
            .map(|name| StickyGrant {
                name,
                min_level: min_levels.get(name).copied().unwrap_or(0),
            })
            .collect()
    }
}
