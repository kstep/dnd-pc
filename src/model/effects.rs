use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    expr::{self, Context, Expr},
    model::{Attribute, Character},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveEffect {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub expr: Option<Expr<Attribute>>,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActiveEffects {
    #[serde(default)]
    effects: Vec<ActiveEffect>,
    /// Computed values set by expression assignments.
    #[serde(skip)]
    overrides: BTreeMap<Attribute, i32>,
}

impl ActiveEffects {
    pub fn effects(&self) -> &[ActiveEffect] {
        &self.effects
    }

    pub fn add(&mut self, mut effect: ActiveEffect, character: &Character) {
        effect.enabled = true;
        self.effects.push(effect);
        self.recompute(character);
    }

    pub fn remove(&mut self, index: usize, character: &Character) -> ActiveEffect {
        let effect = self.effects.remove(index);
        self.recompute(character);
        effect
    }

    /// Update a single field of an effect without recomputing (no expression change).
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
    pub fn recompute(&mut self, character: &Character) {
        self.overrides.clear();

        let exprs: Vec<Expr<Attribute>> = self
            .effects
            .iter()
            .filter(|e| e.enabled)
            .filter_map(|e| e.expr.clone())
            .collect();

        // Mutable wrapper used only here for expression evaluation.
        struct Ctx<'a> {
            character: &'a Character,
            effects: &'a mut ActiveEffects,
        }
        impl Context<Attribute> for Ctx<'_> {
            fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
                self.effects.overrides.insert(var, value);
                Ok(())
            }

            fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
                Ok(self.effects.resolve(self.character, var))
            }
        }

        let mut ctx = Ctx {
            character,
            effects: self,
        };
        for expr in &exprs {
            if let Err(err) = expr.apply(&mut ctx) {
                log::error!("Effect expression error: {err}");
            }
        }
    }

    /// Effective value: override if set, otherwise base from character.
    pub fn resolve(&self, character: &Character, attr: Attribute) -> i32 {
        if let Some(&value) = self.overrides.get(&attr) {
            return value;
        }
        character.resolve(attr).unwrap_or(0)
    }

}
