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
    pub attack_bonus: i32,
    #[serde(default)]
    pub initiative_misc_bonus: i32,
    #[serde(default)]
    pub inspiration: bool,
    #[serde(default = "default_attack_count")]
    pub attack_count: u32,
}

fn default_attack_count() -> u32 {
    1
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
            attack_bonus: 0,
            initiative_misc_bonus: 0,
            inspiration: false,
            attack_count: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct DamageModifiers {
    #[serde(default)]
    pub resistant: bool,
    #[serde(default)]
    pub vulnerable: bool,
    #[serde(default)]
    pub immune: bool,
    #[serde(default)]
    pub reduction: u32,
}

impl DamageModifiers {
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
        self.death_save_successes = 0;
        self.death_save_failures = 0;
    }
}
