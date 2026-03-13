use reactive_stores::Store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Store)]
pub struct CombatStats {
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
    pub initiative_misc_bonus: i32,
    #[serde(default)]
    pub inspiration: bool,
}

impl Default for CombatStats {
    fn default() -> Self {
        Self {
            armor_class: 10,
            speed: 30,
            hp_max: 0,
            hp_current: 0,
            hp_temp: 0,
            death_save_successes: 0,
            death_save_failures: 0,
            initiative_misc_bonus: 0,
            inspiration: false,
        }
    }
}

impl CombatStats {
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
    }
}
