use std::collections::BTreeMap;

use leptos::{html, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        datalist_input::DatalistInput, dice_pool_input::DicePoolInput, icon::Icon,
        toggle_button::ToggleButton,
    },
    effective::EffectiveCharacter,
    expr::Expr,
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
    let expr_input: NodeRef<html::Input> = NodeRef::new();

    let show_dice_pool = RwSignal::new(false);
    let pending_effect = RwSignal::new(Option::<ActiveEffect>::None);
    let pending_rolls = RwSignal::new(BTreeMap::<u32, u32>::new());

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
        <div class="summary-section summary-section-effects">
            <h3 class="summary-section-title">{move_tr!("summary-effects")}</h3>

            // -- Add effect form --
            <div class="summary-list-entry">
                <div class="summary-list-row">
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

                            let effect = ActiveEffect {
                                name,
                                label,
                                description,
                                expr,
                                pool: None,
                                enabled: true,
                            };

                            // Check if expression has dice rolls
                            let rolls = effect.expr.as_ref().map(|e| e.dice_rolls()).unwrap_or_default();
                            if rolls.is_empty() {
                                effects.update(|e| e.add(effect, &store.read()));
                            } else {
                                pending_effect.set(Some(effect));
                                pending_rolls.set(rolls);
                                show_dice_pool.set(true);
                            }

                            effect_label.set(String::new());
                            effect_key.set(None);
                            effect_desc.set(String::new());
                            expr_el.set_value("");
                        }
                    ><Icon name="circle-plus" size=14 /></button>
                    <DatalistInput
                        value=effect_label.get_untracked()
                        placeholder=move_tr!("effect-name")
                        class="summary-list-name"
                        options=effect_options
                        on_input=move |input, resolved| {
                            effect_label.set(input);
                            effect_key.set(resolved.clone());
                            if let Some(name) = resolved {
                                registry.with_effects_index(|index| {
                                    if let Some(eff) = index.get(name.as_str()) {
                                        if let Some(ref expr) = eff.expr
                                            && let Some(el) = expr_input.get()
                                        {
                                            el.set_value(&format!("{expr}"));
                                        }
                                        effect_desc.set(eff.description.clone());
                                    }
                                });
                            }
                        }
                    />
                </div>
                <input type="text" class="summary-list-name summary-item-expr" placeholder=move_tr!("effect-expr") node_ref=expr_input />
            </div>

            // -- Effect list --
            {move || {
                let effects_data = effects.read();
                let effect_list = effects_data.effects();
                if effect_list.is_empty() {
                    return None;
                }
                Some(view! {
                    <div class="summary-list">
                        {effect_list.iter().enumerate().map(|(i, effect)| {
                            let name = effect.label().to_owned();
                            let expr_str = effect.expr.as_ref().map(|expr| format!("{expr}")).unwrap_or_default();
                            let description = effect.description.clone();
                            let enabled = effect.enabled;
                            let is_open = RwSignal::new(false);
                            view! {
                                <div class="summary-list-entry" class:disabled=!enabled>
                                    <div class="summary-list-row">
                                        <ToggleButton
                                            expanded=is_open
                                            on_toggle=move || is_open.update(|v| *v = !*v)
                                        />
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
                                            class="summary-list-name"
                                            prop:value=name
                                            on:change=move |ev| {
                                                let new_name = event_target_value(&ev).trim().to_string();
                                                if new_name.is_empty() { return; }
                                                effects.update(|e| e.update_field(i, |eff| {
                                                    if eff.label.is_some() {
                                                        eff.label = Some(new_name);
                                                    } else {
                                                        eff.name = new_name;
                                                    }
                                                }));
                                            }
                                        />
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
                                    <Show when=move || is_open.get()>
                                        <input
                                            type="text"
                                            class="summary-list-name summary-item-expr"
                                            placeholder=move_tr!("effect-expr")
                                            prop:value=expr_str.clone()
                                            on:change=move |ev| {
                                                let Ok(expr) = parse_expr(&event_target_value(&ev)) else {
                                                    return;
                                                };
                                                effects.update(|effects| {
                                                    effects.update_field(i, |eff| eff.expr = expr);
                                                    effects.recompute(&store.read());
                                                });
                                            }
                                        />
                                        <textarea
                                            class="summary-item-desc"
                                            placeholder=move_tr!("description")
                                            prop:value=description.clone()
                                            on:input=move |ev| {
                                                let val = event_target_value(&ev);
                                                effects.update(|e| e.update_field(i, |eff| eff.description = val));
                                            }
                                        />
                                    </Show>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                })
            }}

            // Dice pool modal (re-created when pending_rolls changes)
            {move || {
                let rolls = pending_rolls.get();
                if rolls.is_empty() {
                    return None;
                }
                Some(view! {
                    <DicePoolInput
                        rolls=rolls
                        show=show_dice_pool
                        on_confirm=move |pool| {
                            pending_effect.update(|pe| {
                                if let Some(effect) = pe.take() {
                                    let mut effect = effect;
                                    effect.pool = Some(pool);
                                    effects.update(|e| e.add(effect, &store.read()));
                                }
                            });
                            pending_rolls.set(BTreeMap::new());
                        }
                    />
                })
            }}
        </div>
    }
}
