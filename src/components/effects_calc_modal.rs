use std::collections::BTreeMap;

use leptos::{html, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{expr_view::ExprDetails, icon::Icon, modal::Modal},
    expr::{self, DicePool},
    model::{Attribute, Character, EffectDefinition, FeatureData, FeatureValue},
};

// --- Read-only context for effect calculation ---

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

// --- Info passed to the modal ---

pub struct EffectsCalcInfo {
    pub title: String,
    pub effects: Vec<EffectDefinition>,
    pub extra_vars: BTreeMap<Attribute, i32>,
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

// --- Effects calculator modal ---

#[component]
pub fn EffectsCalcModal(
    show: RwSignal<bool>,
    info: StoredValue<Option<EffectsCalcInfo>>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();

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

            let effect_views = info
                .effects
                .iter()
                .map(|effect| {
                    let expr = effect.expr.clone();
                    let label = effect.label().to_string();
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

                        // Create NodeRef groups per die type — also
                        // collected into all_groups for the reset button
                        let groups: BTreeMap<u32, Vec<NodeRef<html::Input>>> = rolls
                            .iter()
                            .map(|(&sides, &count)| {
                                let refs: Vec<_> =
                                    (0..count).map(|_| NodeRef::<html::Input>::new()).collect();
                                (sides, refs)
                            })
                            .collect();
                        let groups = StoredValue::new(groups);

                        let total_needed: u32 = rolls.values().copied().sum();
                        let recalc = StoredValue::new(move || {
                            let character = store.read_untracked();
                            let mut ctx = CalcContext {
                                character: &character,
                                extra_vars: &extra_vars,
                            };

                            let pool_map = groups.with_value(|groups| {
                                groups
                                    .iter()
                                    .map(|(&sides, refs)| {
                                        let values: Vec<u32> = refs
                                            .iter()
                                            .filter_map(|node_ref| {
                                                node_ref
                                                    .get()
                                                    .and_then(|el| el.value().parse().ok())
                                                    .filter(|&v: &u32| v >= 1 && v <= sides)
                                            })
                                            .collect();
                                        (sides, values)
                                    })
                                    .collect::<BTreeMap<u32, Vec<u32>>>()
                            });

                            // Only evaluate if all inputs are filled and valid
                            let total_filled: u32 = pool_map.values().map(|v| v.len() as u32).sum();

                            if total_filled == total_needed {
                                let pool: DicePool = pool_map.into();
                                result.set(expr.apply_with_dice(&mut ctx, &pool).ok());
                            } else {
                                result.set(None);
                            }
                        });

                        // Build grouped input views
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

            Some(effect_views)
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
