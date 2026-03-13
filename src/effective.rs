use leptos::prelude::*;
use reactive_stores::Store;

use crate::model::{Ability, ActiveEffects, Attribute, Character, Skill};

/// Reactive read-only view of a character with effects applied.
/// Holds signals, so it's `Copy` and can be used directly in closures.
#[derive(Clone, Copy)]
pub struct EffectiveCharacter {
    store: Store<Character>,
    effects: RwSignal<ActiveEffects>,
}

impl EffectiveCharacter {
    pub fn new(store: Store<Character>, effects: RwSignal<ActiveEffects>) -> Self {
        Self { store, effects }
    }

    pub fn effects(&self) -> RwSignal<ActiveEffects> {
        self.effects
    }

    fn get(&self, attr: Attribute) -> i32 {
        self.effects.read().resolve(&self.store.read(), attr)
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
        self.get(Attribute::Initiative)
    }

    pub fn spell_save_dc(&self, ability: Ability) -> i32 {
        8 + self.proficiency_bonus() + self.ability_modifier(ability)
    }

    pub fn spell_attack_bonus(&self, ability: Ability) -> i32 {
        self.proficiency_bonus() + self.ability_modifier(ability)
    }
}
