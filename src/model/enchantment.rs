use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::rules::{ActionDefinition, Assignment};

/// Magical block embedded in `Item`/`Weapon`/`Armor`. Bundles activatable
/// actions, passive assigns, and an optional charge pool. Empty for mundane
/// gear — skips serialization entirely so existing JSON stays unchanged.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Store)]
pub struct Enchantment {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ActionDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assign: Vec<Assignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charges: Option<Charges>,
}

impl Enchantment {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty() && self.assign.is_empty() && self.charges.is_none()
    }
}

/// Per-instance charge pool. `used` persists across reloads; `max` is
/// recomputed every cycle by `OnGearActive` assigns and clamps `used` if
/// it shrinks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, Store)]
pub struct Charges {
    #[serde(default)]
    pub used: u32,
    #[serde(default)]
    pub max: u32,
}

impl Charges {
    pub fn available(&self) -> u32 {
        self.max.saturating_sub(self.used)
    }
}
