use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    demap,
    expr::{self, Context, DicePool, Expr},
    model::{Ability, Attribute, Character},
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
    #[serde(default)]
    pub scope: Option<Box<str>>,
}

impl ActiveEffect {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    pub fn set_label(&mut self, value: String) {
        self.label = Some(value);
    }
}

impl demap::Named for ActiveEffect {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone)]
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
    /// Per-feature overrides for scoped effects (e.g. SPELL.DC scoped to a
    /// spellcasting feature).
    #[serde(skip)]
    scoped_overrides: BTreeMap<Box<str>, BTreeMap<Attribute, i32>>,
    /// Memoized consumable overrides — evaluated once, then cached
    /// so user edits (e.g. spending temp HP) aren't overwritten.
    /// Persisted so additive effects (HP += X) don't re-apply on reload.
    #[serde(default)]
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
        self.scoped_overrides.clear();

        // Mutable wrapper: borrows overrides mutably and effects immutably.
        struct Ctx<'a> {
            character: &'a Character,
            overrides: &'a mut BTreeMap<Attribute, i32>,
            /// Casting ability from scoped feature (None for unscoped effects)
            casting_ability: Option<Ability>,
        }
        impl Context<Attribute> for Ctx<'_> {
            fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
                if var.is_advantage() {
                    let current = self.resolve(var).unwrap_or(0);
                    self.overrides.insert(var, (current + value).clamp(-1, 1));
                } else {
                    self.overrides.insert(var, value);
                }
                Ok(())
            }

            fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
                if let Some(&value) = self.overrides.get(&var) {
                    return Ok(value);
                }
                match var {
                    Attribute::SpellDc | Attribute::SpellAttack => {
                        let ability = self
                            .casting_ability
                            .ok_or(expr::Error::unsupported_var(var))?;
                        match var {
                            Attribute::SpellDc => Ok(self.character.spell_save_dc(ability)),
                            Attribute::SpellAttack => {
                                Ok(self.character.spell_attack_bonus(ability))
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => Ok(self.character.resolve(var).unwrap_or(0)),
                }
            }
        }

        for effect in self.effects.iter().filter(|e| e.enabled) {
            let Some(ref expr) = effect.expr else {
                continue;
            };

            let casting_ability = effect.scope.as_ref().and_then(|scope| {
                character
                    .feature_data
                    .get(&**scope)
                    .and_then(|e| e.spells.as_ref())
                    .map(|s| s.casting_ability)
            });

            if let Some(scope) = effect.scope.clone() {
                // Scoped effect: evaluate into a temporary map, then merge
                let mut scoped = BTreeMap::new();
                let mut ctx = Ctx {
                    character,
                    overrides: &mut scoped,
                    casting_ability,
                };
                let result = match effect.pool {
                    Some(ref pool) => expr.apply_with_dice(&mut ctx, pool),
                    None => expr.apply(&mut ctx),
                };
                if let Err(error) = result {
                    log::error!("Effect '{}' expression error: {error}", effect.name);
                }
                let entry = self.scoped_overrides.entry(scope).or_default();
                entry.extend(scoped);
            } else {
                // Unscoped effect: evaluate into global overrides
                let mut ctx = Ctx {
                    character,
                    overrides: &mut self.overrides,
                    casting_ability: None,
                };
                let result = match effect.pool {
                    Some(ref pool) => expr.apply_with_dice(&mut ctx, pool),
                    None => expr.apply(&mut ctx),
                };
                if let Err(error) = result {
                    log::error!("Effect '{}' expression error: {error}", effect.name);
                }
            }
        }
        CONSUMABLE_ATTRS.iter().any(|attr| {
            if self.overrides.contains_key(attr) {
                !self.memoized.contains_key(attr)
            } else {
                // Need to clear stale memoized entries when effect is removed
                self.memoized.contains_key(attr)
            }
        })
    }

    /// Effective value: override if set, otherwise base from character.
    pub fn resolve(&self, character: &Character, attr: Attribute) -> i32 {
        if let Some(&value) = self.overrides.get(&attr) {
            return value;
        }
        character.resolve(attr).unwrap_or(0)
    }

    /// Resolve a scoped attribute for a specific feature.
    /// Returns the scoped override if set, otherwise None.
    pub fn resolve_scoped(&self, feature: &str, attr: Attribute) -> Option<i32> {
        self.scoped_overrides
            .get(feature)
            .and_then(|m| m.get(&attr))
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::model::Ability;

    fn effect_with_expr(expr: &str) -> ActiveEffect {
        ActiveEffect {
            name: String::new(),
            label: None,
            description: String::new(),
            expr: Some(expr.parse().unwrap()),
            pool: None,
            enabled: true,
            scope: None,
        }
    }

    #[wasm_bindgen_test]
    fn advantage_additive_clamp() {
        let character = Character::new();
        let mut effects = ActiveEffects::default();

        // Single advantage source → advantage
        effects.add(effect_with_expr("STR.ADV = 1"), &character);
        assert_eq!(
            effects.resolve(&character, Attribute::AbilityAdvantage(Ability::Strength)),
            1
        );

        // Add disadvantage source → cancels to flat
        effects.add(effect_with_expr("STR.ADV = -1"), &character);
        assert_eq!(
            effects.resolve(&character, Attribute::AbilityAdvantage(Ability::Strength)),
            0
        );
    }

    #[wasm_bindgen_test]
    fn advantage_clamps_to_bounds() {
        let character = Character::new();
        let mut effects = ActiveEffects::default();

        // Two advantage sources → still clamped to 1
        effects.add(effect_with_expr("ATK.ADV = 1"), &character);
        effects.add(effect_with_expr("ATK.ADV = 1"), &character);
        assert_eq!(effects.resolve(&character, Attribute::AttackAdvantage), 1);

        // Two disadvantage sources → still clamped to -1
        let mut effects2 = ActiveEffects::default();
        effects2.add(effect_with_expr("DEX.SAVE.ADV = -1"), &character);
        effects2.add(effect_with_expr("DEX.SAVE.ADV = -1"), &character);
        assert_eq!(
            effects2.resolve(&character, Attribute::SaveAdvantage(Ability::Dexterity)),
            -1
        );
    }

    #[wasm_bindgen_test]
    fn advantage_does_not_affect_regular_attrs() {
        let character = Character::new();
        let mut effects = ActiveEffects::default();

        // Regular attribute uses plain assignment (not additive-clamp)
        effects.add(effect_with_expr("AC = 18"), &character);
        assert_eq!(effects.resolve(&character, Attribute::Ac), 18);
    }
}
