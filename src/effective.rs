use leptos::prelude::*;
use reactive_stores::Store;

use crate::model::{Ability, ActiveEffects, Attribute, Character, Skill};

/// Advantage/disadvantage state for a roll type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvantageState {
    Advantage,
    Disadvantage,
    Flat,
}

impl From<i32> for AdvantageState {
    fn from(value: i32) -> Self {
        match value {
            1.. => Self::Advantage,
            ..=-1 => Self::Disadvantage,
            0 => Self::Flat,
        }
    }
}

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

    pub fn attack_bonus(&self) -> i32 {
        self.get(Attribute::AttackBonus)
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

    pub fn ability_advantage(&self, ability: Ability) -> AdvantageState {
        self.get(Attribute::AbilityAdvantage(ability)).into()
    }

    pub fn skill_advantage(&self, skill: Skill) -> AdvantageState {
        self.get(Attribute::SkillAdvantage(skill)).into()
    }

    pub fn save_advantage(&self, ability: Ability) -> AdvantageState {
        self.get(Attribute::SaveAdvantage(ability)).into()
    }

    #[allow(dead_code)]
    pub fn attack_advantage(&self) -> AdvantageState {
        self.get(Attribute::AttackAdvantage).into()
    }
}
