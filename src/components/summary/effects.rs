use leptos::{html, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        datalist_input::DatalistInput,
        expr_args_input::{ExprArgsInput, ExprArgsInputParts},
        icon::Icon,
        modal::Modal,
        toggle_button::ToggleButton,
    },
    effective::EffectiveCharacter,
    expr::{DicePool, Expr},
    model::{ActiveEffect, Attribute, Character},
    rules::RulesRegistry,
};

fn parse_expr(input: &str) -> Result<Option<Expr<Attribute>>, ()> {
    if input.trim().is_empty() {
        return Ok(None);
    }
    input.parse().map(Some).map_err(|error| {
        log::error!("Invalid expression: {error}");
    })
}

#[component]
pub fn EffectsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let effects = eff.effects();
    let registry = expect_context::<RulesRegistry>();

    let effect_label = RwSignal::new(String::new());
    let effect_key = RwSignal::new(Option::<String>::None);
    let effect_desc = RwSignal::new(String::new());
    let effect_scope = RwSignal::new(Option::<Box<str>>::None);
    let expr_input: NodeRef<html::Input> = NodeRef::new();

    // Dice modal state: stores (expr, pending_effect_or_index)
    let show_dice_modal = RwSignal::new(false);
    let pending_expr = RwSignal::new(Option::<Expr<Attribute>>::None);
    let pending_effect = RwSignal::new(Option::<ActiveEffect>::None);
    let reroll_index = RwSignal::new(Option::<usize>::None);
    // Stored parts from ExprArgsInput for dice collection on submit
    let dice_parts: StoredValue<Option<ExprArgsInputParts>> = StoredValue::new(None);

    let open_dice_modal = move |index: Option<usize>, expr: Expr<Attribute>| {
        dice_parts.set_value(None);
        reroll_index.set(index);
        pending_expr.set(Some(expr));
        show_dice_modal.set(true);
    };

    let commit_effect = move |effect: ActiveEffect| {
        effects.update(|active| active.add(effect, &store.read()));
        effect_label.set(String::new());
        effect_key.set(None);
        effect_desc.set(String::new());
        effect_scope.set(None);
        if let Some(el) = expr_input.get() {
            el.set_value("");
        }
    };

    let effect_options = Signal::derive(move || {
        registry.with_effects_index(|index| {
            index
                .values()
                .map(|eff| {
                    (
                        eff.name.clone(),
                        eff.label().to_owned(),
                        eff.description.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });

    view! {
        <div class="summary-section summary-section-effects" id="summary-effects">
            <h3 class="summary-section-title">{move_tr!("summary-effects")}</h3>

            // -- Add effect form --
            <div class="entry-item effect-add-form">
                <button class="btn-icon btn-icon--success"
                    title=move_tr!("effect-add")
                    on:click=move |_| {
                        let Some(expr_el) = expr_input.get() else { return };

                        let label_text = effect_label.get_untracked();
                        let label_text = label_text.trim();
                        if label_text.is_empty() { return; }

                        let Ok(expr) = parse_expr(&expr_el.value()) else {
                            return;
                        };

                        let (name, label) = match effect_key.get_untracked() {
                            Some(key) => (key, Some(label_text.to_owned())),
                            None => (label_text.to_owned(), None),
                        };

                        let description = effect_desc.get_untracked();

                        let scope = effect_scope.get_untracked();
                        let effect = ActiveEffect {
                            name,
                            label,
                            description,
                            expr,
                            pool: None,
                            enabled: true,
                            scope,
                        };

                        // Check if expression has dice rolls
                        let rolls = effect.expr.as_ref().map(|e| e.dice_rolls(&*store.read())).unwrap_or_default();
                        if rolls.is_empty() {
                            commit_effect(effect);
                        } else {
                            let expr = effect.expr.clone().unwrap();
                            pending_effect.set(Some(effect));
                            open_dice_modal(None, expr);
                        }
                    }
                ><Icon name="circle-plus" size=14 /></button>
                <div class="entry-content">
                    <DatalistInput
                        value=effect_label
                        placeholder=move_tr!("effect-name")
                        class="entry-name"
                        options=effect_options
                        on_input=move |input, resolved| {
                            effect_label.set(input);
                            effect_key.set(resolved.clone());
                            if let Some(name) = resolved {
                                registry.with_effects_index(|index| {
                                    if let Some(eff) = index.get(name.as_str()) {
                                        if let Some(el) = expr_input.get() {
                                            let val = eff.expr.as_ref().map(ToString::to_string).unwrap_or_default();
                                            el.set_value(&val);
                                        }
                                        effect_desc.set(eff.description.clone());
                                        effect_scope.set(eff.scope.clone());
                                    }
                                });
                            }
                        }
                    />
                </div>
                <div class="entry-actions" />
                <div class="entry-value">
                    <input type="text" class="summary-item-expr" placeholder=move_tr!("effect-expr") node_ref=expr_input />
                </div>
            </div>

            // -- Effect list --
            {move || {
                let effects_data = effects.read();
                let effect_list = effects_data.effects();
                if effect_list.is_empty() {
                    return None;
                }
                Some(view! {
                    <div class="entry-list">
                        {effect_list.iter().enumerate().map(|(i, effect)| {
                            let name = effect.label().to_owned();
                            let expr_str = effect.expr.as_ref().map(ToString::to_string).unwrap_or_default();
                            let pool_str = effect.pool.as_ref().map(ToString::to_string);
                            let dice_rolls = effect.expr.as_ref().map(|e| e.dice_rolls(&*store.read())).unwrap_or_default();
                            let description = effect.description.clone();
                            let scope = effect.scope.clone();
                            let enabled = effect.enabled;
                            let effect_expr = effect.expr.clone();
                            view! {
                                <div class="entry-item" class:disabled=!enabled>
                                    <ToggleButton />
                                    <div class="entry-content">
                                        <label class="spell-prepared">
                                            <input
                                                type="checkbox"
                                                prop:checked=enabled
                                                on:change=move |_| {
                                                    effects.update(|e| e.toggle(i, &store.read()));
                                                }
                                            />
                                        </label>
                                        <input
                                            type="text"
                                            class="entry-name"
                                            prop:value=name
                                            on:change=move |ev| {
                                                let new_name = event_target_value(&ev).trim().to_string();
                                                if new_name.is_empty() { return; }
                                                effects.update(|e| e.update_field(i, |eff| {
                                                    eff.set_label(new_name);
                                                }));
                                            }
                                        />
                                    </div>
                                    <div class="entry-actions">
                                        <button
                                            class="btn-icon btn-icon--danger"
                                            title=move_tr!("effect-remove")
                                            on:click=move |_| {
                                                effects.update(|e| { e.remove(i, &store.read()); });
                                            }
                                        >
                                            <Icon name="circle-minus" size=14 />
                                        </button>
                                    </div>
                                    {scope.map(|s| view! {
                                        <span class="entry-sublabel">{s.to_string()}</span>
                                    })}
                                        <div class="entry-full-row summary-item-expr-row">
                                            <input
                                                type="text"
                                                class="entry-name summary-item-expr"
                                                placeholder=move_tr!("effect-expr")
                                                prop:value=expr_str.clone()
                                                on:change=move |ev| {
                                                    let Ok(expr) = parse_expr(&event_target_value(&ev)) else {
                                                        return;
                                                    };
                                                    let rolls = expr.as_ref().map(|e| e.dice_rolls(&*store.read())).unwrap_or_default();
                                                    let has_dice = !rolls.is_empty();
                                                    let dice_expr = expr.clone();
                                                    effects.update(|effects| {
                                                        effects.update_field(i, |eff| {
                                                            eff.pool = None;
                                                            eff.expr = expr;
                                                        });
                                                        effects.recompute(&store.read());
                                                    });
                                                    if has_dice
                                                        && let Some(expr) = dice_expr
                                                    {
                                                        open_dice_modal(Some(i), expr);
                                                    }
                                                }
                                            />
                                            {(!dice_rolls.is_empty()).then(|| {
                                                let effect_expr = effect_expr.clone();
                                                view! {
                                                    <button
                                                        class="btn-icon"
                                                        title=move_tr!("effect-reroll")
                                                        on:click=move |_| {
                                                            if let Some(expr) = effect_expr.clone() {
                                                                open_dice_modal(Some(i), expr);
                                                            }
                                                        }
                                                    >
                                                        <Icon name="dices" size=14 />
                                                    </button>
                                                }
                                            })}
                                        </div>
                                        {pool_str.clone().map(|pool| view! {
                                            <span class="entry-sublabel summary-item-dice">
                                                {move_tr!("effect-dice")} ": " {pool}
                                            </span>
                                        })}
                                        <textarea
                                            class="entry-desc"
                                            placeholder=move_tr!("description")
                                            prop:value=description.clone()
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                effects.update(|e| e.update_field(i, |eff| eff.description = val));
                                            }
                                        />
                                </div>
                            }
                        }).collect_view()}
                    </div>
                })
            }}

            // Dice modal using ExprArgsInput
            <Modal show=show_dice_modal title="Dice Rolls">
                {move || {
                    if !show_dice_modal.get() {
                        return None;
                    }
                    let expr = pending_expr.get_untracked()?;

                    let on_ready = move |parts: ExprArgsInputParts| {
                        dice_parts.set_value(Some(parts));
                    };

                    let on_submit = move |event: web_sys::SubmitEvent| {
                        event.prevent_default();
                        let pool: DicePool = dice_parts
                            .with_value(|parts| parts.as_ref().map(|p| p.collect_dice()))
                            .unwrap_or_default();

                        if let Some(effect_index) = reroll_index.get_untracked() {
                            // Re-roll existing effect
                            effects.update(|active| {
                                active.update_field(effect_index, |eff| eff.pool = Some(pool));
                                active.recompute(&store.read());
                            });
                            reroll_index.set(None);
                        } else {
                            // New effect
                            pending_effect.update(|pending| {
                                if let Some(mut effect) = pending.take() {
                                    effect.pool = Some(pool);
                                    commit_effect(effect);
                                }
                            });
                        }
                        show_dice_modal.set(false);
                        pending_expr.set(None);
                    };

                    let expr_input = view! { <ExprArgsInput expr=expr on_ready /> };
                    Some(view! {
                        <form class="dice-pool-form" on:submit=on_submit>
                            {expr_input}
                            <div class="dice-pool-footer">
                                <button type="submit" class="btn-confirm">
                                    <Icon name="check" size=16 />
                                    " Confirm"
                                </button>
                            </div>
                        </form>
                    }.into_any())
                }}
            </Modal>
        </div>
    }
}
