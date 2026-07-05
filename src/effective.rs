use leptos::prelude::*;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    expr::{self, Context, Eval as _, VarGroup},
    model::{
        Ability, AbilityGroup, ActiveEffects, Attribute, AttributeGroup, Character,
        DamageModifiers, DamageType, DmgGroup, Item, Skill, SkillGroup, ToolGroup, intern,
    },
};

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

impl Context<Attribute, i32> for EffectiveCharacter {
    fn assign(&mut self, var: Attribute, _value: i32) -> Result<(), expr::Error> {
        Err(expr::Error::read_only_var(var))
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        Ok(self.get(var))
    }
}

// Static groups iterate `VARIANTS` lazily; Tool reads tools per row via
// the Copy + 'static `Store<Character>` captured in a from_fn closure.
impl expr::ResolveGroup<AttributeGroup> for EffectiveCharacter {
    fn resolve_group<'a>(
        &'a self,
        grp: &AttributeGroup,
    ) -> Box<dyn Iterator<Item = Vec<Attribute>> + 'a> {
        use strum::VariantArray;
        match grp {
            AttributeGroup::Ability => Box::new(
                Ability::VARIANTS
                    .iter()
                    .copied()
                    .enumerate()
                    .map(AbilityGroup::make_row),
            ),
            AttributeGroup::Skill => Box::new(
                Skill::VARIANTS
                    .iter()
                    .copied()
                    .enumerate()
                    .map(SkillGroup::make_row),
            ),
            AttributeGroup::Dmg => Box::new(
                DamageType::VARIANTS
                    .iter()
                    .copied()
                    .enumerate()
                    .map(DmgGroup::make_row),
            ),
            AttributeGroup::Tool => {
                let store = self.store;
                let mut i = 0usize;
                Box::new(std::iter::from_fn(move || {
                    let character = store.read_untracked();
                    let name = intern(&character.tools.iter().nth(i)?.name);
                    let row = ToolGroup::make_row((i, name));
                    i += 1;
                    Some(row)
                }))
            }
        }
    }
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

    /// Evaluate a weapon item's derived attack expression with effect
    /// overlays applied. `ATK` resolves through the overlay, so global
    /// buffs (Bless, Bardic Inspiration) are included automatically.
    pub fn weapon_attack_bonus(&self, item: &Item) -> i32 {
        item.attack_expr()
            .map(|expr| expr.eval(self).unwrap_or(0))
            .unwrap_or(0)
    }

    pub fn attack_count(&self) -> i32 {
        self.get(Attribute::Attacks)
    }

    pub fn hp_max(&self) -> i32 {
        self.get(Attribute::MaxHp)
    }

    pub fn initiative(&self) -> i32 {
        self.get(Attribute::Initiative)
    }

    pub fn spell_save_dc(&self, ability: Ability, feature: &str) -> i32 {
        let effects = self.effects.read();
        let global_delta = effects.global_override(Attribute::SpellDc).unwrap_or(0);
        if let Some(dc) = effects.resolve_scoped(feature, Attribute::SpellDc) {
            return dc + global_delta;
        }
        let base = 8 + self.proficiency_bonus() + self.ability_modifier(ability);
        base + global_delta
    }

    pub fn spell_attack_bonus(&self, ability: Ability, feature: &str) -> i32 {
        let effects = self.effects.read();
        let global_delta = effects.global_override(Attribute::SpellAttack).unwrap_or(0);
        if let Some(atk) = effects.resolve_scoped(feature, Attribute::SpellAttack) {
            return atk + global_delta;
        }
        let base = self.proficiency_bonus() + self.ability_modifier(ability);
        base + global_delta
    }

    pub fn spell_attack_advantage(&self, feature: &str) -> AdvantageState {
        let effects = self.effects.read();
        effects
            .resolve_scoped(feature, Attribute::SpellAttackAdvantage)
            .or_else(|| effects.global_override(Attribute::SpellAttackAdvantage))
            .unwrap_or(0)
            .into()
    }

    pub fn ability_advantage(&self, ability: Ability) -> AdvantageState {
        self.get(Attribute::AbilityAdvantage(ability)).into()
    }

    pub fn skill_advantage(&self, skill: Skill) -> AdvantageState {
        let skill_adv = self.get(Attribute::SkillAdvantage(skill));
        let ability_adv = self.get(Attribute::AbilityAdvantage(skill.ability()));
        (skill_adv + ability_adv).clamp(-1, 1).into()
    }

    pub fn save_advantage(&self, ability: Ability) -> AdvantageState {
        self.get(Attribute::SaveAdvantage(ability)).into()
    }

    #[allow(dead_code)]
    pub fn attack_advantage(&self) -> AdvantageState {
        self.get(Attribute::AttackAdvantage).into()
    }

    /// Effective damage modifiers: character base merged with effect overrides.
    pub fn damage_modifiers(&self) -> DamageModifiers {
        let character = self.store.read();
        let effects = self.effects.read();
        let mut result = character.damage_modifiers.clone();

        for damage_type in DamageType::iter() {
            let resistant = effects.global_override(Attribute::Resistance(damage_type));
            let vulnerable = effects.global_override(Attribute::Vulnerability(damage_type));
            let immune = effects.global_override(Attribute::Immunity(damage_type));
            let reduction = effects.global_override(Attribute::DamageReduction(damage_type));

            if resistant.is_some()
                || vulnerable.is_some()
                || immune.is_some()
                || reduction.is_some()
            {
                let entry = result.entry(damage_type).or_default();
                if let Some(value) = resistant {
                    entry.resistant = value != 0;
                }
                if let Some(value) = vulnerable {
                    entry.vulnerable = value != 0;
                }
                if let Some(value) = immune {
                    entry.immune = value != 0;
                }
                if let Some(value) = reduction {
                    entry.reduction = value.max(0) as u32;
                }
                if !entry.is_active() {
                    result.remove(&damage_type);
                }
            }
        }

        result
    }
}
