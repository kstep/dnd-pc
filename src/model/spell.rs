use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::{
    constvec::ConstVec,
    model::{Ability, SpellSlotPool},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct SpellData {
    pub casting_ability: Ability,
    #[serde(default)]
    pub caster_coef: u32,
    #[serde(default)]
    pub pool: SpellSlotPool,
    #[serde(default)]
    pub spells: Vec<Spell>,
    #[serde(default)]
    pub known: Option<Vec<Spell>>,
}

impl SpellData {
    pub fn cantrips(&self) -> impl Iterator<Item = &Spell> {
        self.spells.iter().filter(|s| s.level == 0)
    }

    pub fn spells(&self) -> impl Iterator<Item = &Spell> {
        self.spells.iter().filter(|s| s.level > 0)
    }

    pub fn is_two_tier(&self) -> bool {
        self.known.is_some()
    }

    /// Effective caster level when one class contributes to the pool.
    pub fn single_caster_level(class_level: u32, coef: u32) -> u32 {
        let threshold = if coef == 3 { 3 } else { 1 };
        if class_level < threshold {
            0
        } else {
            class_level.div_ceil(coef)
        }
    }
}

impl Default for SpellData {
    fn default() -> Self {
        Self {
            casting_ability: Ability::Intelligence,
            caster_coef: 0,
            pool: SpellSlotPool::default(),
            spells: Vec::new(),
            known: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct SpellSlotLevel {
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub used: u32,
}

impl SpellSlotLevel {
    pub fn available(&self) -> u32 {
        self.total.saturating_sub(self.used)
    }

    pub fn is_available(&self) -> bool {
        self.available() > 0
    }

    pub fn is_empty(&self) -> bool {
        self.available() == 0
    }
}

/// Per-character spell slots keyed by pool, each pool carrying up to 9 slot
/// levels. Transparent over the underlying `BTreeMap`, accessible as a map via
/// `Deref`/`DerefMut`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Store)]
#[serde(transparent)]
pub struct SpellSlots(
    #[serde(deserialize_with = "crate::serde_util::deserialize_map_dropping_nulls")]
    BTreeMap<SpellSlotPool, ConstVec<SpellSlotLevel, 9>>,
);

impl Deref for SpellSlots {
    type Target = BTreeMap<SpellSlotPool, ConstVec<SpellSlotLevel, 9>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SpellSlots {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<BTreeMap<SpellSlotPool, ConstVec<SpellSlotLevel, 9>>> for SpellSlots {
    fn from(map: BTreeMap<SpellSlotPool, ConstVec<SpellSlotLevel, 9>>) -> Self {
        Self(map)
    }
}

impl SpellSlots {
    /// Read a single slot. Returns `SpellSlotLevel::default()` if pool or level
    /// is absent.
    pub fn get_slot(&self, pool: SpellSlotPool, level: u32) -> SpellSlotLevel {
        self.0
            .get(&pool)
            .and_then(|slots| slots.get((level - 1) as usize))
            .copied()
            .unwrap_or_default()
    }

    /// Iterate slot levels 1..=9 for the given pool, yielding each level and
    /// its current `SpellSlotLevel` (defaulted when absent).
    pub fn iter_pool(
        &self,
        pool: SpellSlotPool,
    ) -> impl Iterator<Item = (u32, SpellSlotLevel)> + '_ {
        (1..=9u32).map(move |level| (level, self.get_slot(pool, level)))
    }

    /// Pools that have at least one slot level with non-zero `total`.
    pub fn active_pools(&self) -> impl Iterator<Item = SpellSlotPool> + '_ {
        self.0
            .iter()
            .filter(|(_, slots)| slots.iter().any(|slot| slot.total > 0))
            .map(|(&pool, _)| pool)
    }

    /// Low-level write: overwrite `total` for each slot level in `pool` from
    /// `totals`. Extra levels beyond `totals.len()` are left untouched. `used`
    /// counters are not modified.
    pub fn set_totals(&mut self, pool: SpellSlotPool, totals: &[u32]) {
        if totals.is_empty() {
            return;
        }
        let slot_entry = self.0.entry(pool).or_default();
        for (i, entry) in slot_entry.iter_mut().enumerate() {
            let table_total = totals.get(i).copied().unwrap_or(0);
            entry.total = table_total;
        }
    }

    /// Zero `used` on every slot in every pool.
    pub fn reset_used(&mut self) {
        self.reset_used_where(|_pool| true);
    }

    /// Zero `used` on every slot in pools matching `predicate`.
    pub fn reset_used_where(&mut self, mut predicate: impl FnMut(&SpellSlotPool) -> bool) {
        for (pool, slots) in self.0.iter_mut() {
            if predicate(pool) {
                for slot in slots.iter_mut() {
                    slot.used = 0;
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FreeUses {
    #[serde(default)]
    pub used: u32,
    #[serde(default)]
    pub max: u32,
}

impl FreeUses {
    pub fn available(&self) -> u32 {
        self.max.saturating_sub(self.used)
    }

    pub fn is_available(&self) -> bool {
        self.available() > 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Spell {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sticky: bool,
    #[serde(default)]
    pub cost: u32,
    #[serde(default)]
    pub free_uses: Option<FreeUses>,
}

impl Spell {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn set_label(&mut self, value: String) {
        self.label = Some(value);
    }

    /// Upsert `free_uses.max`, clamping `used` if it exceeds the new max.
    /// Creates `free_uses` if absent (with `used = 0`).
    pub fn set_free_uses_max(&mut self, new_max: u32) {
        match &mut self.free_uses {
            Some(fu) => {
                fu.max = new_max;
                if fu.used > new_max {
                    fu.used = new_max;
                }
            }
            None => {
                self.free_uses = Some(FreeUses {
                    used: 0,
                    max: new_max,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn spell_slots_deserialize_skips_null_pool_entries() {
        // Null pool values arrive via the Firestore merge-tombstone path.
        // Dropping them on read keeps Character loadable.
        let value = json!({"0": null});
        let slots: SpellSlots = serde_json::from_value(value).expect("must deserialize");
        assert!(
            slots.is_empty(),
            "null pools must be dropped, got {slots:?}"
        );
    }

    #[test]
    fn spell_slots_deserialize_mixed_null_and_valid_pools() {
        let value = json!({
            "0": [{"total": 4, "used": 0}],
            "1": null,
        });
        let slots: SpellSlots = serde_json::from_value(value).expect("must deserialize");
        assert_eq!(slots.len(), 1, "only valid pool must remain");
        assert!(slots.contains_key(&SpellSlotPool::Arcane));
    }
}
