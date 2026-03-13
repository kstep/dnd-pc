use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    expr::{self, Context, Expr},
    model::{Ability, Attribute, Character, Skill},
};

pub struct EffectiveCharacter<'a> {
    character: &'a Character,
    effects: &'a mut ActiveEffects,
}

impl<'a> EffectiveCharacter<'a> {
    pub fn new(character: &'a Character, effects: &'a mut ActiveEffects) -> Self {
        Self { character, effects }
    }

    pub fn character(&self) -> &Character {
        self.character
    }

    fn get(&self, attr: Attribute) -> i32 {
        self.effects.resolve(self.character, attr)
    }

    pub fn ability_score(&self, ability: Ability) -> i32 {
        self.get(Attribute::Ability(ability))
    }

    pub fn ability_modifier(&self, ability: Ability) -> i32 {
        self.get(Attribute::Modifier(ability))
    }

    pub fn saving_throw_bonus(&self, ability: Ability) -> i32 {
        self.get(Attribute::SavingThrow(ability))
    }

    pub fn skill_bonus(&self, skill: Skill) -> i32 {
        self.get(Attribute::Skill(skill))
    }

    pub fn proficiency_bonus(&self) -> i32 {
        self.get(Attribute::ProfBonus)
    }

    pub fn armor_class(&self) -> i32 {
        self.get(Attribute::Ac)
    }

    pub fn speed(&self) -> i32 {
        self.get(Attribute::Speed)
    }

    pub fn hp_max(&self) -> i32 {
        self.get(Attribute::MaxHp)
    }

    pub fn initiative(&self) -> i32 {
        self.character.combat.initiative_misc_bonus as i32
            + self.ability_modifier(Ability::Dexterity)
    }

    pub fn spell_save_dc(&self, ability: Ability) -> i32 {
        8 + self.proficiency_bonus() + self.ability_modifier(ability)
    }

    pub fn spell_attack_bonus(&self, ability: Ability) -> i32 {
        self.proficiency_bonus() + self.ability_modifier(ability)
    }
}

impl Context<Attribute> for EffectiveCharacter<'_> {
    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        self.effects.overrides.insert(var, value);
        Ok(())
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        Ok(self.effects.resolve(self.character, var))
    }
}

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

        let mut ctx = EffectiveCharacter::new(character, self);
        for expr in &exprs {
            if let Err(err) = expr.apply(&mut ctx) {
                log::error!("Effect expression error: {err}");
            }
        }
    }

    /// Whether any enabled effect modifies this attribute.
    pub fn has_override(&self, attr: Attribute) -> bool {
        self.overrides.contains_key(&attr)
    }

    /// Effective value: override if set, otherwise base from character.
    pub fn resolve(&self, character: &Character, attr: Attribute) -> i32 {
        if let Some(&value) = self.overrides.get(&attr) {
            return value;
        }
        character.resolve(attr).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }
}
