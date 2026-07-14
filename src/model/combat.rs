use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::model::{DamageType, Sense};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct CombatStats {
    #[serde(default)]
    pub concentrating: Option<String>,
    #[serde(default)]
    pub armor_class: u32,
    #[serde(default)]
    pub speed: u32,
    #[serde(default)]
    pub hp_max: u32,
    #[serde(default)]
    pub hp_current: u32,
    #[serde(default)]
    pub hp_temp: u32,
    #[serde(default)]
    pub death_save_successes: u8,
    #[serde(default)]
    pub death_save_failures: u8,
    #[serde(default)]
    pub attack_bonus: i32,
    #[serde(default)]
    pub initiative_misc_bonus: i32,
    #[serde(default)]
    pub inspiration: bool,
    #[serde(default = "default_attack_count")]
    pub attack_count: u32,
    #[serde(default = "default_attunement_max")]
    pub attunement_max: u32,
}

fn default_attack_count() -> u32 {
    1
}

pub fn default_attunement_max() -> u32 {
    3
}

impl Default for CombatStats {
    fn default() -> Self {
        Self {
            concentrating: None,
            armor_class: 10,
            speed: 30,
            hp_max: 0,
            hp_current: 0,
            hp_temp: 0,
            death_save_successes: 0,
            death_save_failures: 0,
            attack_bonus: 0,
            initiative_misc_bonus: 0,
            inspiration: false,
            attack_count: 1,
            attunement_max: default_attunement_max(),
        }
    }
}

/// Per-damage-type handling: resistance/vulnerability/immunity flags plus a
/// flat reduction amount subtracted before multiplicative modifiers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct DamageModifier {
    #[serde(default)]
    pub resistant: bool,
    #[serde(default)]
    pub vulnerable: bool,
    #[serde(default)]
    pub immune: bool,
    #[serde(default)]
    pub reduction: u32,
}

impl DamageModifier {
    pub fn is_active(&self) -> bool {
        self.resistant || self.vulnerable || self.immune || self.reduction > 0
    }

    pub fn modify(&self, mut amount: u32) -> u32 {
        if self.immune {
            return 0;
        }

        amount = amount.saturating_sub(self.reduction);

        if self.resistant {
            amount /= 2;
        }

        if self.vulnerable {
            amount *= 2;
        }

        amount
    }
}

/// Per-character damage modifier table keyed by damage type. Transparent over
/// the underlying `BTreeMap` for serde, accessible as a map via `Deref`. Setter
/// methods auto-remove entries that become inactive (all flags false and
/// reduction zero), keeping the wire format compact.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DamageModifiers(
    #[serde(deserialize_with = "crate::serde_util::deserialize_map_dropping_nulls")]
    BTreeMap<DamageType, DamageModifier>,
);

impl Deref for DamageModifiers {
    type Target = BTreeMap<DamageType, DamageModifier>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DamageModifiers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl IntoIterator for DamageModifiers {
    type IntoIter = std::collections::btree_map::IntoIter<DamageType, DamageModifier>;
    type Item = (DamageType, DamageModifier);

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl DamageModifiers {
    /// Read a single entry. Returns `DamageModifier::default()` when absent.
    pub fn get_entry(&self, dt: DamageType) -> DamageModifier {
        self.0.get(&dt).copied().unwrap_or_default()
    }

    pub fn is_resistant(&self, dt: DamageType) -> bool {
        self.0.get(&dt).is_some_and(|m| m.resistant)
    }

    pub fn is_vulnerable(&self, dt: DamageType) -> bool {
        self.0.get(&dt).is_some_and(|m| m.vulnerable)
    }

    pub fn is_immune(&self, dt: DamageType) -> bool {
        self.0.get(&dt).is_some_and(|m| m.immune)
    }

    pub fn reduction(&self, dt: DamageType) -> u32 {
        self.0.get(&dt).map_or(0, |m| m.reduction)
    }

    pub fn set_resistant(&mut self, dt: DamageType, enabled: bool) {
        self.set_flag(dt, enabled, |m| &mut m.resistant);
    }

    pub fn set_vulnerable(&mut self, dt: DamageType, enabled: bool) {
        self.set_flag(dt, enabled, |m| &mut m.vulnerable);
    }

    pub fn set_immune(&mut self, dt: DamageType, enabled: bool) {
        self.set_flag(dt, enabled, |m| &mut m.immune);
    }

    pub fn set_reduction(&mut self, dt: DamageType, value: u32) {
        let entry = self.0.entry(dt).or_default();
        entry.reduction = value;
        if !entry.is_active() {
            self.0.remove(&dt);
        }
    }

    /// Toggle a bool field on the entry for `dt`. Removes the entry if it
    /// becomes inactive.
    pub fn toggle(&mut self, dt: DamageType, field: impl FnOnce(&mut DamageModifier) -> &mut bool) {
        let entry = self.0.entry(dt).or_default();
        let flag = field(entry);
        *flag = !*flag;
        if !entry.is_active() {
            self.0.remove(&dt);
        }
    }

    fn set_flag(
        &mut self,
        dt: DamageType,
        enabled: bool,
        field: impl FnOnce(&mut DamageModifier) -> &mut bool,
    ) {
        let entry = self.0.entry(dt).or_default();
        *field(entry) = enabled;
        if !entry.is_active() {
            self.0.remove(&dt);
        }
    }
}

/// Per-character senses, each a range in feet (0 = the character lacks it).
/// Granted by features/effects through `SENSE.<X>` assign expressions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Store)]
#[serde(default)]
pub struct Senses {
    pub darkvision: u32,
    pub blindsight: u32,
    pub tremorsense: u32,
    pub truesight: u32,
}

impl Senses {
    pub fn get(&self, sense: Sense) -> u32 {
        match sense {
            Sense::Darkvision => self.darkvision,
            Sense::Blindsight => self.blindsight,
            Sense::Tremorsense => self.tremorsense,
            Sense::Truesight => self.truesight,
        }
    }

    pub fn set(&mut self, sense: Sense, feet: u32) {
        match sense {
            Sense::Darkvision => self.darkvision = feet,
            Sense::Blindsight => self.blindsight = feet,
            Sense::Tremorsense => self.tremorsense = feet,
            Sense::Truesight => self.truesight = feet,
        }
    }
}

impl CombatStats {
    /// Copy play state fields (hp_current/temp, death saves, concentrating,
    /// inspiration) from `other` into `self`, clamping `hp_current` to the
    /// current (possibly shrunken) `hp_max`. Used by rebuild's merge phase
    /// after fresh derived values have been computed.
    pub fn merge_play_state(&mut self, other: &Self) {
        self.hp_current = other.hp_current.min(self.hp_max);
        self.hp_temp = other.hp_temp;
        self.death_save_successes = other.death_save_successes;
        self.death_save_failures = other.death_save_failures;
        self.concentrating = other.concentrating.clone();
        self.inspiration = other.inspiration;
    }

    pub fn damage(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }

        let amount = if self.hp_temp > 0 {
            let temp_absorb = self.hp_temp.min(amount);
            self.hp_temp -= temp_absorb;
            amount - temp_absorb
        } else {
            amount
        };

        self.hp_current = self.hp_current.saturating_sub(amount);
    }

    pub fn heal(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }

        self.hp_current = (self.hp_current + amount).min(self.hp_max);
        self.death_save_successes = 0;
        self.death_save_failures = 0;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn senses_get_set_dispatch() {
        let mut senses = Senses::default();
        assert_eq!(senses.get(Sense::Darkvision), 0);
        senses.set(Sense::Darkvision, 60);
        senses.set(Sense::Tremorsense, 30);
        assert_eq!(senses.get(Sense::Darkvision), 60);
        assert_eq!(senses.get(Sense::Tremorsense), 30);
        assert_eq!(senses.get(Sense::Blindsight), 0);
        assert_eq!(senses.get(Sense::Truesight), 0);
        senses.set(Sense::Darkvision, 0);
        assert_eq!(senses.get(Sense::Darkvision), 0);
    }

    #[test]
    fn damage_modifiers_deserialize_drops_null_tombstones() {
        // Firestore merge-tombstone path — see src/storage/diff.rs
        // `merged_*_with_null_deserializes_as_tombstone_drop` tests for
        // the full end-to-end rationale.
        let value = json!({
            "0": { "resistant": true, "vulnerable": false, "immune": false, "reduction": 0 },
            "3": null,
        });
        let modifiers: DamageModifiers = serde_json::from_value(value).expect("must deserialize");
        assert!(modifiers.is_resistant(DamageType::Acid));
        assert_eq!(modifiers.len(), 1);
    }
}
