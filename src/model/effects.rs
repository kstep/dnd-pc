use std::{cmp::Ordering, collections::BTreeMap};

use serde::{Deserialize, Serialize};

use crate::{
    demap,
    expr::{self, Context, DicePool},
    model::{Ability, Attribute, Character, CharacterCore, Charges, Expr, GearRef, WeaponEffect},
    rules::{FeaturesView, WhenCondition},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EffectRange {
    Caster,
    #[default]
    Touch,
    Feet(u32),
}

impl Ord for EffectRange {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Caster, Self::Caster) | (Self::Touch, Self::Touch) => Ordering::Equal,
            (Self::Caster, _) => Ordering::Less,
            (_, Self::Caster) => Ordering::Greater,
            (Self::Touch, _) => Ordering::Less,
            (_, Self::Touch) => Ordering::Greater,
            (Self::Feet(a), Self::Feet(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for EffectRange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl EffectRange {
    pub fn can_target_self(self) -> bool {
        matches!(self, EffectRange::Caster | EffectRange::Touch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EffectDuration {
    #[default]
    Instant,
    Rounds(u32),
    Forever,
}

impl Ord for EffectDuration {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Instant, Self::Instant) | (Self::Forever, Self::Forever) => Ordering::Equal,
            (Self::Instant, _) | (_, Self::Forever) => Ordering::Less,
            (Self::Forever, _) | (_, Self::Instant) => Ordering::Greater,
            (Self::Rounds(a), Self::Rounds(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for EffectDuration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A lightweight effect definition carrying a name and expression.
/// Used on `SpellDefinition` for damage/healing formulas; designed to be
/// reusable for feature effects, weapon effects, etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectDefinition {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub expr: Option<Expr>,
    #[serde(default)]
    pub range: EffectRange,
    #[serde(default)]
    pub duration: EffectDuration,
    #[serde(default)]
    pub stackable: bool,
    #[serde(default)]
    pub scope: Option<String>,
}

impl EffectDefinition {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

impl From<&WeaponEffect> for EffectDefinition {
    fn from(effect: &WeaponEffect) -> Self {
        Self {
            name: effect.name.clone(),
            label: None,
            expr: Some(effect.expr.clone()),
            range: EffectRange::default(),
            duration: EffectDuration::default(),
            stackable: false,
            scope: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveEffect {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub expr: Option<Expr>,
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

/// Catalog entry — locale-less template for predefined effects (loaded from
/// `public/data/effects.json`). Runtime label/description come from the
/// parallel `EffectsLocaleMap` overlay; user-edited fields live on
/// `ActiveEffect` after the template is materialized.
#[derive(Debug, Clone, Deserialize)]
pub struct EffectTemplate {
    pub name: Box<str>,
    #[serde(default)]
    pub expr: Option<Expr>,
    #[serde(default)]
    pub scope: Option<Box<str>>,
}

impl demap::Named for EffectTemplate {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Default)]
pub struct EffectsIndex(pub BTreeMap<Box<str>, EffectTemplate>);

impl<'de> Deserialize<'de> for EffectsIndex {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        demap::named_map(deserializer).map(Self)
    }
}

/// Attributes whose values are "consumed" — set once by an effect
/// and then managed by the user (e.g. temp HP spent by damage).
const CONSUMABLE_ATTRS: [Attribute; 2] = [Attribute::Hp, Attribute::TempHp];

/// Mutable wrapper that layers scoped overrides on top of global ones.
/// Optionally bound to a gear slot for read-only `Charges*` / `Quantity`
/// resolution during gear `OnEffect` evaluation.
struct Ctx<'a> {
    character: &'a Character,
    gear: Option<GearRef>,
    global: &'a mut BTreeMap<Attribute, i32>,
    scoped: Option<&'a mut BTreeMap<Attribute, i32>>,
    casting_ability: Option<Ability>,
}

impl Ctx<'_> {
    fn gear_charges(&self) -> Option<&Charges> {
        let gear = self.gear?;
        match gear {
            GearRef::Item(i) => self
                .character
                .equipment
                .items
                .get(i)?
                .magic
                .charges
                .as_ref(),
            GearRef::Weapon(i) => self
                .character
                .equipment
                .weapons
                .get(i)?
                .magic
                .charges
                .as_ref(),
            GearRef::Armor(i) => self
                .character
                .equipment
                .armors
                .get(i)?
                .magic
                .charges
                .as_ref(),
        }
    }

    fn gear_quantity(&self) -> Option<u32> {
        let gear = self.gear?;
        match gear {
            GearRef::Item(i) => self.character.equipment.items.get(i).map(|x| x.quantity),
            GearRef::Weapon(i) => self.character.equipment.weapons.get(i).map(|x| x.quantity),
            GearRef::Armor(_) => Some(1),
        }
    }
}

impl AsRef<CharacterCore> for Ctx<'_> {
    fn as_ref(&self) -> &CharacterCore {
        &self.character.core
    }
}

impl Context<Attribute, i32> for Ctx<'_> {
    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        // Gear-local attrs are read-only in OnEffect.
        if matches!(
            var,
            Attribute::Charges
                | Attribute::ChargesMax
                | Attribute::ChargesUsed
                | Attribute::Quantity,
        ) {
            log::warn!("{var:?} is read-only in OnEffect overlay");
            return Ok(());
        }
        let value = if var.is_advantage() {
            let current = self.resolve(var).unwrap_or(0);
            (current + value).clamp(-1, 1)
        } else {
            value
        };
        let target = if var.is_scoped() {
            self.scoped.as_deref_mut().unwrap_or(&mut *self.global)
        } else {
            &mut *self.global
        };
        target.insert(var, value);
        Ok(())
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        // Gear-local reads when bound to a gear slot.
        match var {
            Attribute::Charges => {
                return Ok(self.gear_charges().map(|c| c.available()).unwrap_or(0) as i32);
            }
            Attribute::ChargesMax => {
                return Ok(self.gear_charges().map(|c| c.max).unwrap_or(0) as i32);
            }
            Attribute::ChargesUsed => {
                return Ok(self.gear_charges().map(|c| c.used).unwrap_or(0) as i32);
            }
            Attribute::Quantity => return Ok(self.gear_quantity().unwrap_or(0) as i32),
            _ => {}
        }
        // SpellDc / SpellAttack: scoped is an absolute per-feature override,
        // global is an additive delta applied to every feature (gear focus +1
        // to all spells writes here). Read semantics differ by ctx:
        //
        // - Scoped feature ctx: returns scoped or base — WITHOUT global delta.
        //   `SPELL.DC += 1` writes capture only the feature's contribution into scoped;
        //   global is layered on top at the final consumer.
        // - Gear ctx (no scope, no casting ability): returns global delta as
        //   accumulator so `SPELL.DC += 1` reads 0 and writes 1 to global.
        if matches!(var, Attribute::SpellDc | Attribute::SpellAttack) {
            let scoped_abs = self.scoped.as_ref().and_then(|s| s.get(&var)).copied();
            return match (scoped_abs, self.casting_ability) {
                (Some(abs), _) => Ok(abs),
                (None, Some(ability)) => match var {
                    Attribute::SpellDc => Ok(self.character.spell_save_dc(ability)),
                    Attribute::SpellAttack => Ok(self.character.spell_attack_bonus(ability)),
                    _ => unreachable!(),
                },
                (None, None) => Ok(self.global.get(&var).copied().unwrap_or(0)),
            };
        }
        // Check scoped first, then global, then character base
        if let Some(ref scoped) = self.scoped
            && let Some(&value) = scoped.get(&var)
        {
            return Ok(value);
        }
        if let Some(&value) = self.global.get(&var) {
            return Ok(value);
        }
        Ok(self.character.resolve(var).unwrap_or(0))
    }
}

fn eval_user_effects(
    character: &Character,
    effects: &[ActiveEffect],
    overrides: &mut BTreeMap<Attribute, i32>,
    scoped_overrides: &mut BTreeMap<Box<str>, BTreeMap<Attribute, i32>>,
) {
    for effect in effects.iter().filter(|effect| effect.enabled) {
        let Some(ref expr) = effect.expr else {
            continue;
        };
        let casting_ability = effect.scope.as_ref().and_then(|scope| {
            character
                .features
                .get(&**scope)
                .and_then(|e| e.spells.as_ref())
                .map(|s| s.casting_ability)
        });
        let mut ctx = Ctx {
            character,
            gear: None,
            global: overrides,
            scoped: effect
                .scope
                .clone()
                .map(|scope| scoped_overrides.entry(scope).or_default()),
            casting_ability,
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

fn eval_features_on_effect(
    character: &Character,
    feat_index: FeaturesView<'_>,
    overrides: &mut BTreeMap<Attribute, i32>,
    scoped_overrides: &mut BTreeMap<Box<str>, BTreeMap<Attribute, i32>>,
) {
    for feature in character.features.iter() {
        let Some(def) = feat_index.get(&feature.name) else {
            continue;
        };
        let Some(assigns) = def.assign.as_deref() else {
            continue;
        };
        let scope_name = feature.name.clone();
        let casting_ability = character
            .features
            .get(&*scope_name)
            .and_then(|e| e.spells.as_ref())
            .map(|s| s.casting_ability);
        let mut ctx = Ctx {
            character,
            gear: None,
            global: overrides,
            scoped: Some(scoped_overrides.entry(scope_name).or_default()),
            casting_ability,
        };
        for assign in assigns.iter().filter(|a| a.when == WhenCondition::OnEffect) {
            if let Err(error) = assign.expr.apply(&mut ctx) {
                log::debug!("Feature '{}' OnEffect assign error: {error}", feature.name);
            }
        }
    }
}

fn eval_gear_on_effect(character: &Character, overrides: &mut BTreeMap<Attribute, i32>) {
    let mut run = |gear: GearRef, assigns: &[crate::rules::feature::Assignment]| {
        let mut ctx = Ctx {
            character,
            gear: Some(gear),
            global: overrides,
            scoped: None,
            casting_ability: None,
        };
        for assign in assigns.iter().filter(|a| a.when == WhenCondition::OnEffect) {
            if let Err(error) = assign.expr.apply(&mut ctx) {
                log::debug!("Gear {gear:?} OnEffect assign error: {error}");
            }
        }
    };

    for (i, item) in character.equipment.items.iter().enumerate() {
        if !item.is_active() || item.magic.assign.is_empty() {
            continue;
        }
        run(GearRef::Item(i), &item.magic.assign);
    }
    for (i, weapon) in character.equipment.weapons.iter().enumerate() {
        if !weapon.is_active() || weapon.magic.assign.is_empty() {
            continue;
        }
        run(GearRef::Weapon(i), &weapon.magic.assign);
    }
    for (i, armor) in character.equipment.armors.iter().enumerate() {
        if !armor.is_active() || armor.magic.assign.is_empty() {
            continue;
        }
        run(GearRef::Armor(i), &armor.magic.assign);
    }
}

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

    /// Check if an active effect with the given name exists.
    pub fn has_effect(&self, name: &str) -> bool {
        self.effects.iter().any(|effect| effect.name == name)
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

    /// Push an effect onto the list. Does NOT recompute — the caller's
    /// reactive scope will trigger `recompute` on the next layout effect
    /// cycle (since `effects` is observed by the EffectiveCharacter pass).
    pub fn add(&mut self, effect: ActiveEffect) {
        self.effects.push(effect);
    }

    /// Remove the effect at `index`. Does NOT recompute (see `add`).
    pub fn remove(&mut self, index: usize) -> ActiveEffect {
        self.effects.remove(index)
    }

    /// Update a single field of an effect without recomputing (no expression
    /// change).
    pub fn update_field(&mut self, index: usize, f: impl FnOnce(&mut ActiveEffect)) {
        if let Some(effect) = self.effects.get_mut(index) {
            f(effect);
        }
    }

    /// Flip the `enabled` flag on the effect at `index`. Does NOT recompute.
    pub fn toggle(&mut self, index: usize) {
        if let Some(effect) = self.effects.get_mut(index) {
            effect.enabled = !effect.enabled;
        }
    }

    /// Evaluate all enabled expressions. Must be called after
    /// deserialization and after any mutation.
    ///
    /// Three sources contribute to the overlay:
    /// 1. User effects (`self.effects` filtered by `enabled`).
    /// 2. Feature `OnEffect` assigns from the catalog (transient passive
    ///    bonuses while a feature is in `features.list`).
    /// 3. Active gear `OnEffect` assigns from
    ///    `equipment.{items,weapons,armors}` filtered by `is_active()`.
    pub fn recompute(&mut self, character: &Character, feat_index: FeaturesView<'_>) -> bool {
        self.overrides.clear();
        self.scoped_overrides.clear();

        let Self {
            effects,
            overrides,
            scoped_overrides,
            ..
        } = self;

        // Layer order: most-permanent → least-permanent. Features ground the
        // overlay first, gear stacks on top of them, user effects (toggled
        // buffs) run last so their absolute writes win on conflict.
        eval_features_on_effect(character, feat_index, overrides, scoped_overrides);
        eval_gear_on_effect(character, overrides);
        eval_user_effects(character, effects, overrides, scoped_overrides);

        CONSUMABLE_ATTRS.iter().any(|attr| {
            if self.overrides.contains_key(attr) {
                !self.memoized.contains_key(attr)
            } else {
                // Need to clear stale memoized entries when effect is removed
                self.memoized.contains_key(attr)
            }
        })
    }

    /// Returns a global override for the given attribute, if any.
    pub fn global_override(&self, attr: Attribute) -> Option<i32> {
        self.overrides.get(&attr).copied()
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
    use crate::{
        model::{Ability, FeatureData, SpellData, SpellSlotPool},
        rules::feature::EMPTY_FEATURES_INDEX,
    };

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

    fn recompute(effects: &mut ActiveEffects, character: &Character) {
        effects.recompute(character, FeaturesView::from_natural(&EMPTY_FEATURES_INDEX));
    }

    #[wasm_bindgen_test]
    fn advantage_additive_clamp() {
        let character = Character::new();
        let mut effects = ActiveEffects::default();

        // Single advantage source → advantage
        effects.add(effect_with_expr("STR.ADV = 1"));
        recompute(&mut effects, &character);
        assert_eq!(
            effects.resolve(&character, Attribute::AbilityAdvantage(Ability::Strength)),
            1
        );

        // Add disadvantage source → cancels to flat
        effects.add(effect_with_expr("STR.ADV = -1"));
        recompute(&mut effects, &character);
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
        effects.add(effect_with_expr("ATK.ADV = 1"));
        effects.add(effect_with_expr("ATK.ADV = 1"));
        recompute(&mut effects, &character);
        assert_eq!(effects.resolve(&character, Attribute::AttackAdvantage), 1);

        // Two disadvantage sources → still clamped to -1
        let mut effects2 = ActiveEffects::default();
        effects2.add(effect_with_expr("DEX.SAVE.ADV = -1"));
        effects2.add(effect_with_expr("DEX.SAVE.ADV = -1"));
        recompute(&mut effects2, &character);
        assert_eq!(
            effects2.resolve(&character, Attribute::SaveAdvantage(Ability::Dexterity)),
            -1
        );
    }

    fn scoped_effect(scope: &str, expr: &str) -> ActiveEffect {
        ActiveEffect {
            name: String::new(),
            label: None,
            description: String::new(),
            expr: Some(expr.parse().unwrap()),
            pool: None,
            enabled: true,
            scope: Some(scope.into()),
        }
    }

    fn character_with_spellcasting(feature: &str, ability: Ability) -> Character {
        let mut character = Character::new();
        character.features.insert(
            feature.into(),
            FeatureData {
                spells: Some(SpellData {
                    casting_ability: ability,
                    caster_coef: 1,
                    pool: SpellSlotPool::default(),
                    spells: Vec::new(),
                    known: None,
                }),
                ..Default::default()
            },
        );
        character
    }

    #[wasm_bindgen_test]
    fn scoped_effects_stack() {
        let feature = "Spellcasting (Sorcerer)";
        let character = character_with_spellcasting(feature, Ability::Charisma);
        let base_dc = character.spell_save_dc(Ability::Charisma);
        let mut effects = ActiveEffects::default();

        effects.add(scoped_effect(feature, "SPELL.DC += 1"));
        recompute(&mut effects, &character);
        assert_eq!(
            effects.resolve_scoped(feature, Attribute::SpellDc),
            Some(base_dc + 1),
        );

        effects.add(scoped_effect(feature, "SPELL.DC += 1"));
        recompute(&mut effects, &character);
        assert_eq!(
            effects.resolve_scoped(feature, Attribute::SpellDc),
            Some(base_dc + 2),
        );
    }

    #[wasm_bindgen_test]
    fn scoped_effect_forwards_non_spell_attrs_to_global() {
        let feature = "Spellcasting (Sorcerer)";
        let character = character_with_spellcasting(feature, Ability::Charisma);
        let base_ac = character.resolve(Attribute::Ac).unwrap_or(0);
        let base_dc = character.spell_save_dc(Ability::Charisma);
        let mut effects = ActiveEffects::default();

        // Scoped effect with both spell and non-spell attributes
        effects.add(scoped_effect(feature, "SPELL.DC += 1; AC += 1"));
        recompute(&mut effects, &character);

        // Spell DC goes to scoped storage
        assert_eq!(
            effects.resolve_scoped(feature, Attribute::SpellDc),
            Some(base_dc + 1),
        );
        // AC forwards to global overrides
        assert_eq!(effects.resolve(&character, Attribute::Ac), base_ac + 1);
        // AC is NOT in scoped storage
        assert_eq!(effects.resolve_scoped(feature, Attribute::Ac), None);
    }

    #[wasm_bindgen_test]
    fn scoped_effect_sees_unscoped_overrides() {
        let feature = "Spellcasting (Sorcerer)";
        let character = character_with_spellcasting(feature, Ability::Charisma);
        let base_ac = character.resolve(Attribute::Ac).unwrap_or(0);
        let mut effects = ActiveEffects::default();

        // Unscoped effect sets AC
        effects.add(effect_with_expr("AC += 2"));
        // Scoped effect layers on top
        effects.add(scoped_effect(feature, "AC += 1"));
        recompute(&mut effects, &character);

        // Should see base + 2 + 1 = base + 3
        assert_eq!(effects.resolve(&character, Attribute::Ac), base_ac + 3);
    }

    #[wasm_bindgen_test]
    fn global_spell_dc_stacks_on_top_of_feature_scope() {
        // Magic focus: gear writes `SPELL.DC += 1` (no scope, no
        // casting_ability) → stored as global delta, applied to every
        // feature on read.
        let feature = "Spellcasting (Sorcerer)";
        let character = character_with_spellcasting(feature, Ability::Charisma);
        let base_dc = character.spell_save_dc(Ability::Charisma);
        let mut effects = ActiveEffects::default();

        // Simulate gear OnEffect by writing a non-scoped effect.
        effects.add(effect_with_expr("SPELL.DC += 1"));
        recompute(&mut effects, &character);

        // Without scoped feature override: base + global delta.
        assert_eq!(
            effects.resolve_scoped(feature, Attribute::SpellDc),
            None,
            "scoped should be empty — feature didn't write its own override",
        );
        assert_eq!(
            effects.global_override(Attribute::SpellDc),
            Some(1),
            "global should store the delta",
        );

        // Adding a feature scoped override on top: stacks additively.
        effects.add(scoped_effect(feature, "SPELL.DC += 1"));
        recompute(&mut effects, &character);
        // scoped contains the absolute (base + 1) captured at write time;
        // global delta still 1; final consumer sees scoped + global = base + 2.
        assert_eq!(
            effects.resolve_scoped(feature, Attribute::SpellDc),
            Some(base_dc + 1),
        );
        assert_eq!(effects.global_override(Attribute::SpellDc), Some(1));
    }

    #[wasm_bindgen_test]
    fn advantage_does_not_affect_regular_attrs() {
        let character = Character::new();
        let mut effects = ActiveEffects::default();

        // Regular attribute uses plain assignment (not additive-clamp)
        effects.add(effect_with_expr("AC = 18"));
        recompute(&mut effects, &character);
        assert_eq!(effects.resolve(&character, Attribute::Ac), 18);
    }
}
