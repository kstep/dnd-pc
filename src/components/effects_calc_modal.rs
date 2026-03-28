use std::collections::BTreeMap;

use leptos::{html, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{expr_view::ExprDetails, icon::Icon, modal::Modal},
    effective::EffectiveCharacter,
    expr::{self, DicePool, Expr, Op},
    model::{
        ActiveEffect, ActiveEffects, Attribute, Character, EffectDefinition, EffectDuration,
        EffectRange, FeatureData, FeatureValue,
    },
};

// --- Read-only context for effect calculation (display only) ---

struct CalcContext<'a> {
    character: &'a Character,
    extra_vars: &'a BTreeMap<Attribute, i32>,
}

impl expr::Context<Attribute, i32> for CalcContext<'_> {
    fn assign(&mut self, _var: Attribute, _value: i32) -> Result<(), expr::Error> {
        Ok(())
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        if let Some(&value) = self.extra_vars.get(&var) {
            return Ok(value);
        }
        self.character.resolve(var)
    }
}

// --- Mutable context for instant effect application ---

struct ApplyContext<'a> {
    character: &'a mut Character,
    extra_vars: &'a BTreeMap<Attribute, i32>,
}

impl expr::Context<Attribute, i32> for ApplyContext<'_> {
    fn assign(&mut self, var: Attribute, value: i32) -> Result<(), expr::Error> {
        self.character.assign(var, value)
    }

    fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
        if let Some(&value) = self.extra_vars.get(&var) {
            return Ok(value);
        }
        self.character.resolve(var)
    }
}

// --- Info passed to the modal ---

pub struct EffectsCalcInfo {
    pub title: String,
    pub effects: Vec<EffectDefinition>,
    pub extra_vars: BTreeMap<Attribute, i32>,
    pub spell_name: String,
    pub feature_name: String,
}

/// Populate `extra_vars` with resource field values (POINTS/POINTS_MAX) from
/// a feature's data entry. Finds the first Points or Die field.
pub fn inject_resource_vars(extra_vars: &mut BTreeMap<Attribute, i32>, entry: &FeatureData) {
    for field in &entry.fields {
        match &field.value {
            FeatureValue::Points { used, max } => {
                extra_vars.insert(Attribute::Points, (*max - *used) as i32);
                extra_vars.insert(Attribute::PointsMax, *max as i32);
                return;
            }
            FeatureValue::Die { die, used } => {
                extra_vars.insert(Attribute::Points, (die.amount - *used) as i32);
                extra_vars.insert(Attribute::PointsMax, die.amount as i32);
                return;
            }
            _ => {}
        }
    }
}

/// Check whether all Caster effects in the list have no dice rolls.
/// When true, effects can be applied immediately without showing the modal.
pub fn all_self_effects_diceless(
    effects: &[EffectDefinition],
    character: &Character,
    extra_vars: &BTreeMap<Attribute, i32>,
) -> bool {
    let ctx = CalcContext {
        character,
        extra_vars,
    };
    effects
        .iter()
        .filter(|effect| effect.range == EffectRange::Caster)
        .all(|effect| effect.expr.dice_rolls(&ctx).is_empty())
}

/// Check whether any non-stackable Caster effect already exists in active
/// effects.
pub fn has_non_stackable_duplicate(
    effects: &[EffectDefinition],
    active_effects: &ActiveEffects,
    spell_name: &str,
) -> bool {
    let any_non_stackable = effects
        .iter()
        .any(|effect| effect.range == EffectRange::Caster && !effect.stackable);
    any_non_stackable && active_effects.has_effect(spell_name)
}

/// Build a combined expression from Caster effects matching a duration filter.
fn build_self_expr(
    effects: &[EffectDefinition],
    filter: fn(EffectDuration) -> bool,
) -> Option<Expr<Attribute>> {
    let combined = effects
        .iter()
        .filter(|effect| effect.range == EffectRange::Caster && filter(effect.duration))
        .map(|effect| effect.expr.to_string())
        .collect::<Vec<_>>()
        .join("; ");
    if combined.is_empty() {
        None
    } else {
        combined.parse().ok()
    }
}

/// Replace contextual PushVar ops with PushConst so the expression is
/// self-contained when stored as an ActiveEffect.
fn bind_extra_vars(
    expr: &Expr<Attribute>,
    extra_vars: &BTreeMap<Attribute, i32>,
) -> Expr<Attribute> {
    expr.map(|op| match op {
        Op::PushVar(var) if extra_vars.contains_key(var) => Op::PushConst(extra_vars[var]),
        other => *other,
    })
}

/// Apply all Caster effects immediately (no dice, no modal).
/// Instant effects are applied directly to the character;
/// persistent effects create an ActiveEffect (unless blocked by stackable).
pub fn apply_self_effects_now(
    effects: &[EffectDefinition],
    spell_name: &str,
    feature_name: &str,
    extra_vars: &BTreeMap<Attribute, i32>,
    store: &Store<Character>,
    active_effects: RwSignal<ActiveEffects>,
) {
    if let Some(expr) = build_self_expr(effects, |duration| duration == EffectDuration::Instant) {
        store.update(|character| {
            let mut ctx = ApplyContext {
                character,
                extra_vars,
            };
            if let Err(error) = expr.apply(&mut ctx) {
                log::error!("Instant effect error: {error}");
            }
        });
    }

    if let Some(expr) = build_self_expr(effects, |duration| duration != EffectDuration::Instant) {
        // Skip if non-stackable and already active
        if has_non_stackable_duplicate(effects, &active_effects.read_untracked(), spell_name) {
            return;
        }
        let expr = bind_extra_vars(&expr, extra_vars);
        // Use explicit scope from effect definition if set, otherwise feature_name
        let effect_scope = effects
            .iter()
            .find_map(|effect| effect.scope.as_deref())
            .unwrap_or(feature_name);
        let scope = if effect_scope.is_empty() {
            None
        } else {
            Some(effect_scope.into())
        };
        let effect = ActiveEffect {
            name: spell_name.to_string(),
            label: None,
            description: String::new(),
            expr: Some(expr),
            pool: None,
            enabled: true,
            scope,
        };
        active_effects.update(|active| active.add(effect, &store.read()));
    }
}

/// Collect dice values from NodeRef groups into a raw pool map.
/// Used only by this module's inline dice inputs (which use NodeRefs for
/// live result recalculation, unlike the signal-based ExprArgsInput).
fn collect_dice_pool(groups: &BTreeMap<u32, Vec<NodeRef<html::Input>>>) -> BTreeMap<u32, Vec<u32>> {
    groups
        .iter()
        .map(|(&sides, refs)| {
            let values: Vec<u32> = refs
                .iter()
                .filter_map(|node_ref| node_ref.get().and_then(|el| el.value().parse().ok()))
                .collect();
            (sides, values)
        })
        .collect()
}

// --- Effects calculator modal ---

#[component]
pub fn EffectsCalcModal(
    show: RwSignal<bool>,
    info: StoredValue<Option<EffectsCalcInfo>>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let effects = eff.effects();

    let title = Signal::derive(move || {
        info.with_value(|info| info.as_ref().map(|i| i.title.clone()).unwrap_or_default())
    });

    // Build effect views when modal opens
    let content = move || {
        if !show.get() {
            return None;
        }

        info.with_value(|info| {
            let info = info.as_ref()?;
            let character = store.read();

            let ctx = CalcContext {
                character: &character,
                extra_vars: &info.extra_vars,
            };

            // Check if any effect is self-targeting and can be applied
            let has_self_effects = info
                .effects
                .iter()
                .any(|effect| effect.range == EffectRange::Caster)
                && !has_non_stackable_duplicate(
                    &info.effects,
                    &effects.read_untracked(),
                    &info.spell_name,
                );

            // Collect self-targeting dice groups for the "Apply Effect" button
            type DiceGroups = Vec<StoredValue<BTreeMap<u32, Vec<NodeRef<html::Input>>>>>;
            let self_dice_groups: StoredValue<DiceGroups> = StoredValue::new(Vec::new());

            // Build separate instant and persistent expressions
            let (instant_expr, persistent_expr) = if has_self_effects {
                let build_expr = |filter: fn(EffectDuration) -> bool| -> Option<Expr<Attribute>> {
                    let combined = info
                        .effects
                        .iter()
                        .filter(|effect| {
                            effect.range == EffectRange::Caster && filter(effect.duration)
                        })
                        .map(|effect| effect.expr.to_string())
                        .collect::<Vec<_>>()
                        .join("; ");
                    if combined.is_empty() {
                        None
                    } else {
                        combined.parse().ok()
                    }
                };
                (
                    build_expr(|duration| duration == EffectDuration::Instant),
                    build_expr(|duration| duration != EffectDuration::Instant),
                )
            } else {
                (None, None)
            };
            let instant_expr = StoredValue::new(instant_expr);
            let persistent_expr = StoredValue::new(persistent_expr);
            let extra_vars_copy = StoredValue::new(info.extra_vars.clone());
            let spell_name = StoredValue::new(info.spell_name.clone());
            // Use explicit scope from effect definition if set, otherwise feature_name
            let effect_scope = info
                .effects
                .iter()
                .find_map(|effect| effect.scope.clone())
                .unwrap_or_else(|| info.feature_name.clone());
            let feature_name = StoredValue::new(effect_scope);

            let effect_views = info
                .effects
                .iter()
                .map(|effect| {
                    let expr = effect.expr.clone();
                    let label = effect.label().to_string();
                    let is_self = effect.range == EffectRange::Caster;
                    let rolls = effect.expr.dice_rolls(&ctx);

                    if rolls.is_empty() {
                        // No dice — evaluate immediately
                        let result = effect.expr.eval_lenient(&ctx).ok();
                        let result_view = result.map_or_else(
                            || view! { <span class="effects-calc-error">"\u{2014}"</span> }.into_any(),
                            |v| v.into_any(),
                        );
                        view! {
                            <div class="effects-calc-row">
                                <div class="effects-calc-header">
                                    <span class="effects-calc-label">{label}</span>
                                    <strong class="effects-calc-result">{result_view}</strong>
                                </div>
                                <ExprDetails expr />
                            </div>
                        }
                        .into_any()
                    } else {
                        // Has dice — build inputs and live result
                        let result = RwSignal::new(None::<i32>);
                        let formula_expr = effect.expr.clone();
                        let expr = effect.expr.clone();
                        let extra_vars = info.extra_vars.clone();

                        let groups: BTreeMap<u32, Vec<NodeRef<html::Input>>> = rolls
                            .iter()
                            .map(|(&sides, &count)| {
                                let refs: Vec<_> =
                                    (0..count).map(|_| NodeRef::<html::Input>::new()).collect();
                                (sides, refs)
                            })
                            .collect();
                        let groups = StoredValue::new(groups);

                        // Track self-targeting dice groups for Apply Effect
                        if is_self {
                            self_dice_groups.update_value(|v| v.push(groups));
                        }

                        let total_needed: u32 = rolls.values().copied().sum();
                        let recalc = StoredValue::new(move || {
                            let character = store.read_untracked();
                            let mut ctx = CalcContext {
                                character: &character,
                                extra_vars: &extra_vars,
                            };

                            let pool_map = groups.with_value(collect_dice_pool);

                            let total_filled: u32 =
                                pool_map.values().map(|v| v.len() as u32).sum();

                            if total_filled == total_needed {
                                let pool: DicePool = pool_map.into();
                                result.set(expr.apply_with_dice(&mut ctx, &pool).ok());
                            } else {
                                result.set(None);
                            }
                        });

                        let mut first_input = true;
                        let group_views = groups.with_value(|groups| {
                            groups
                                .iter()
                                .map(|(&sides, refs)| {
                                    let input_views = refs
                                        .iter()
                                        .map(|&node_ref| {
                                            let is_first = first_input;
                                            first_input = false;
                                            view! {
                                                <input
                                                    type="number"
                                                    min=1
                                                    max=sides
                                                    required
                                                    autofocus=is_first
                                                    class="dice-pool-value"
                                                    node_ref=node_ref
                                                    on:input=move |_| recalc.with_value(|f| f())
                                                />
                                            }
                                        })
                                        .collect_view();
                                    view! {
                                        <div class="dice-pool-group">
                                            <span class="dice-pool-label">"d" {sides}</span>
                                            <div class="dice-pool-inputs">{input_views}</div>
                                        </div>
                                    }
                                })
                                .collect_view()
                        });

                        let reset = move |_: web_sys::MouseEvent| {
                            groups.with_value(|groups| {
                                let mut first = true;
                                for refs in groups.values() {
                                    for node_ref in refs {
                                        if let Some(el) = node_ref.get() {
                                            el.set_value("");
                                            if first {
                                                let _ = el.focus();
                                                first = false;
                                            }
                                        }
                                    }
                                }
                            });
                            result.set(None);
                        };

                        view! {
                            <div class="effects-calc-row">
                                <div class="effects-calc-header">
                                    <span class="effects-calc-label">{label}</span>
                                    <strong class="effects-calc-result">
                                        {move || result.get().map_or_else(
                                            || view! { <span class="effects-calc-error">"\u{2014}"</span> }.into_any(),
                                            |v| v.into_any(),
                                        )}
                                    </strong>
                                    <button
                                        type="button"
                                        class="effects-calc-reset"
                                        title=move_tr!("reset")
                                        on:click=reset
                                    >
                                        <Icon name="rotate-ccw" size=14 />
                                    </button>
                                </div>
                                <ExprDetails expr=formula_expr />
                                <div class="dice-pool-groups">{group_views}</div>
                            </div>
                        }
                        .into_any()
                    }
                })
                .collect_view();

            // "Apply Effect" button for self-targeting spells
            let apply_button = has_self_effects.then(|| {
                let apply_effect = move |_: web_sys::MouseEvent| {
                    // Collect dice pool from all self-targeting effect inputs
                    let mut merged_pool = BTreeMap::<u32, Vec<u32>>::new();
                    self_dice_groups.with_value(|dice_groups| {
                        for groups in dice_groups {
                            groups.with_value(|groups| {
                                for (sides, values) in collect_dice_pool(groups) {
                                    merged_pool.entry(sides).or_default().extend(values);
                                }
                            });
                        }
                    });

                    let pool = if merged_pool.is_empty() {
                        None
                    } else {
                        Some(DicePool::from(merged_pool))
                    };

                    // Instant effects: apply directly to character with extra vars
                    if let Some(expr) = instant_expr.get_value() {
                        extra_vars_copy.with_value(|extra_vars| {
                            store.update(|character| {
                                let mut ctx = ApplyContext {
                                    character,
                                    extra_vars,
                                };
                                let result = match &pool {
                                    Some(pool) => expr.apply_with_dice(&mut ctx, pool),
                                    None => expr.apply(&mut ctx),
                                };
                                if let Err(error) = result {
                                    log::error!("Instant effect error: {error}");
                                }
                            });
                        });
                    }

                    // Persistent effects: create ActiveEffect with substituted vars
                    if let Some(expr) = persistent_expr.get_value() {
                        let name = spell_name.get_value();
                        let scope = feature_name.with_value(|fname| {
                            if fname.is_empty() {
                                None
                            } else {
                                Some(fname.clone().into_boxed_str())
                            }
                        });

                        let expr = extra_vars_copy
                            .with_value(|extra_vars| bind_extra_vars(&expr, extra_vars));

                        let effect = ActiveEffect {
                            name,
                            label: None,
                            description: String::new(),
                            expr: Some(expr),
                            pool,
                            enabled: true,
                            scope,
                        };

                        effects.update(|active| active.add(effect, &store.read()));
                    }

                    show.set(false);
                };

                view! {
                    <div class="effects-calc-footer">
                        <button
                            type="button"
                            class="btn-confirm"
                            on:click=apply_effect
                        >
                            <Icon name="shield-plus" size=16 />
                            " " {move_tr!("apply-effect")}
                        </button>
                    </div>
                }
            });

            Some(view! {
                {effect_views}
                {apply_button}
            })
        })
    };

    view! {
        <Modal show=show title=title>
            <div class="effects-calc">
                {content}
            </div>
        </Modal>
    }
}
