use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    demap,
    expr::{self, Context, DicePool, Expr},
    model::{Attribute, Character},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveEffect {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub expr: Option<Expr<Attribute>>,
    #[serde(skip)]
    pub pool: Option<DicePool>,
    #[serde(default)]
    pub enabled: bool,
}

impl ActiveEffect {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

impl demap::Named for ActiveEffect {
    fn name(&self) -> &str {
        &self.name
    }
}

pub struct EffectsIndex(pub BTreeMap<Box<str>, ActiveEffect>);

impl<'de> Deserialize<'de> for EffectsIndex {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        demap::named_map(deserializer).map(Self)
    }
}

/// Attributes whose values are "consumed" — set once by an effect
/// and then managed by the user (e.g. temp HP spent by damage).
const CONSUMABLE_ATTRS: [Attribute; 2] = [Attribute::Hp, Attribute::TempHp];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActiveEffects {
    #[serde(default)]
    effects: Vec<ActiveEffect>,
    /// Computed values set by expression assignments.
    #[serde(skip)]
    overrides: BTreeMap<Attribute, i32>,
    /// Memoized consumable overrides — evaluated once, then cached
    /// so user edits (e.g. spending temp HP) aren't overwritten.
    #[serde(skip)]
    memoized: BTreeMap<Attribute, i32>,
}

impl ActiveEffects {
    pub fn effects(&self) -> &[ActiveEffect] {
        &self.effects
    }

    /// Propagate consumable overrides (Hp, TempHp) to the character.
    /// Values are memoized: only written on the first recompute that
    /// produces them, so user edits aren't overwritten. Returns true
    /// if any values were propagated.
    pub fn propagate(&mut self, character: &mut Character) -> bool {
        let mut changed = false;
        for attr in CONSUMABLE_ATTRS {
            if let Some(value) = self.overrides.remove(&attr) {
                if self.memoized.insert(attr, value).is_none() {
                    let _ = character.assign(attr, value);
                    changed = true;
                }
            } else {
                self.memoized.remove(&attr);
            }
        }
        changed
    }

    pub fn add(&mut self, effect: ActiveEffect, character: &Character) {
        let needs_recompute = effect.enabled && effect.expr.is_some();
        self.effects.push(effect);
        if needs_recompute {
            self.recompute(character);
        }
    }

    pub fn remove(&mut self, index: usize, character: &Character) -> ActiveEffect {
        let effect = self.effects.remove(index);
        self.recompute(character);
        effect
    }

    /// Update a single field of an effect without recomputing (no expression
    /// change).
    pub fn update_field(&mut self, index: usize, f: impl FnOnce(&mut ActiveEffect)) {
        if let Some(effect) = self.effects.get_mut(index) {
            f(effect);
        }
    }

    pub fn toggle(&mut self, index: usize, character: &Character) {
        if let Some(effect) = self.effects.get_mut(index) {
            effect.enabled = !effect.enabled;
        }
        self.recompute(character);
    }

    /// Evaluate all enabled expressions. Must be called after
    /// deserialization and after any mutation.
    pub fn recompute(&mut self, character: &Character) -> bool {
        self.overrides.clear();

        // Mutable wrapper: borrows overrides mutably and effects immutably.
        struct Ctx<'a> {
            character: &'a Character,
            overrides: &'a mut BTreeMap<Attribute, i32>,
        }
        impl Context<Attribute> for Ctx<'_> {
            fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
                self.overrides.insert(var, value);
                Ok(())
            }

            fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
                if let Some(&value) = self.overrides.get(&var) {
                    return Ok(value);
                }
                Ok(self.character.resolve(var).unwrap_or(0))
            }
        }

        let mut ctx = Ctx {
            character,
            overrides: &mut self.overrides,
        };
        for effect in self.effects.iter().filter(|e| e.enabled) {
            let Some(ref expr) = effect.expr else {
                continue;
            };
            let result = match effect.pool {
                Some(ref pool) => expr.apply_with_dice(&mut ctx, pool),
                None => expr.apply(&mut ctx),
            };
            if let Err(error) = result {
                log::error!("Effect expression error: {error}");
            }
        }
        CONSUMABLE_ATTRS
            .iter()
            .any(|attr| self.overrides.contains_key(attr) && !self.memoized.contains_key(attr))
    }

    /// Effective value: override if set, otherwise base from character.
    pub fn resolve(&self, character: &Character, attr: Attribute) -> i32 {
        if let Some(&value) = self.overrides.get(&attr) {
            return value;
        }
        character.resolve(attr).unwrap_or(0)
    }
}
