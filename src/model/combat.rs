use std::{
    collections::BTreeMap,
    fmt,
    ops::{Deref, DerefMut},
};

use reactive_stores::Store;
use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, Visitor, value::MapAccessDeserializer},
};

use crate::model::{DamageType, Sense, SpeedMode};

/// Default walking speed in feet (most species).
pub const DEFAULT_SPEED: u32 = 30;

/// Per-mode movement speeds in feet. Walking speed is the primary mode; the
/// others are 0 unless a species/feature/effect sets them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Store)]
pub struct Speed {
    pub walk: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub fly: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub swim: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub climb: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub burrow: u32,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

impl Speed {
    pub fn get(&self, mode: SpeedMode) -> u32 {
        match mode {
            SpeedMode::Walk => self.walk,
            SpeedMode::Fly => self.fly,
            SpeedMode::Swim => self.swim,
            SpeedMode::Climb => self.climb,
            SpeedMode::Burrow => self.burrow,
        }
    }

    pub fn set(&mut self, mode: SpeedMode, value: u32) {
        match mode {
            SpeedMode::Walk => self.walk = value,
            SpeedMode::Fly => self.fly = value,
            SpeedMode::Swim => self.swim = value,
            SpeedMode::Climb => self.climb = value,
            SpeedMode::Burrow => self.burrow = value,
        }
    }
}

/// Legacy characters store `speed` as a bare walking-speed number; current
/// ones store a per-mode map. Accept both transparently — no schema bump.
impl<'de> Deserialize<'de> for Speed {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct SpeedVisitor;

        impl<'de> Visitor<'de> for SpeedVisitor {
            type Value = Speed;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a walking-speed number or a per-mode speed map")
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Speed, E> {
                Ok(Speed {
                    walk: value as u32,
                    ..Default::default()
                })
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Speed, E> {
                Ok(Speed {
                    walk: value.max(0) as u32,
                    ..Default::default()
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Speed, A::Error> {
                #[derive(Deserialize)]
                struct Raw {
                    #[serde(default)]
                    walk: u32,
                    #[serde(default)]
                    fly: u32,
                    #[serde(default)]
                    swim: u32,
                    #[serde(default)]
                    climb: u32,
                    #[serde(default)]
                    burrow: u32,
                }
                let raw = Raw::deserialize(MapAccessDeserializer::new(map))?;
                Ok(Speed {
                    walk: raw.walk,
                    fly: raw.fly,
                    swim: raw.swim,
                    climb: raw.climb,
                    burrow: raw.burrow,
                })
            }
        }

        de.deserialize_any(SpeedVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Store)]
pub struct CombatStats {
    #[serde(default)]
    pub concentrating: Option<String>,
    #[serde(default)]
    pub armor_class: u32,
    #[serde(default)]
    pub speed: Speed,
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
            speed: Speed {
                walk: DEFAULT_SPEED,
                ..Default::default()
            },
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
    use strum::IntoEnumIterator;

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
    fn speed_deserializes_legacy_number_into_walk() {
        let speed: Speed = serde_json::from_value(json!(30)).expect("must deserialize");
        assert_eq!(
            speed,
            Speed {
                walk: 30,
                ..Default::default()
            }
        );
    }

    #[test]
    fn speed_deserializes_full_map() {
        let value = json!({ "walk": 30, "fly": 60, "swim": 15, "climb": 10, "burrow": 5 });
        let speed: Speed = serde_json::from_value(value).expect("must deserialize");
        assert_eq!(
            speed,
            Speed {
                walk: 30,
                fly: 60,
                swim: 15,
                climb: 10,
                burrow: 5
            }
        );
    }

    #[test]
    fn speed_deserializes_partial_map_defaulting_missing() {
        let speed: Speed = serde_json::from_value(json!({ "walk": 25 })).expect("must deserialize");
        assert_eq!(
            speed,
            Speed {
                walk: 25,
                ..Default::default()
            }
        );
    }

    #[test]
    fn speed_serialize_skips_zero_non_walk_fields() {
        let value = serde_json::to_value(Speed {
            walk: 30,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(value, json!({ "walk": 30 }));
    }

    #[test]
    fn speed_get_set_round_trips_each_mode() {
        let mut speed = Speed::default();
        for mode in SpeedMode::iter() {
            speed.set(mode, 42);
            assert_eq!(speed.get(mode), 42);
        }
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
