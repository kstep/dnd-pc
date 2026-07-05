use reactive_stores::Store;
use serde::{Deserialize, Serialize};

use crate::{
    model::{DamageType, Expr},
    rules::{ActionDefinition, Assignment},
};

/// One always-visible damage line of an item (weapon dice, bonus riders).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DamageEffect {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub damage_type: Option<DamageType>,
    #[serde(default)]
    pub expr: Expr,
}

/// Effects block embedded in `Item`: damage lines, activatable actions,
/// passive assigns, and an optional charge pool. Empty for plain gear —
/// skips serialization entirely.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Store)]
pub struct ItemEffects {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub damage: Vec<DamageEffect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ActionDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assign: Vec<Assignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charges: Option<Charges>,
}

impl ItemEffects {
    pub fn is_empty(&self) -> bool {
        self.damage.is_empty()
            && self.actions.is_empty()
            && self.assign.is_empty()
            && self.charges.is_none()
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
