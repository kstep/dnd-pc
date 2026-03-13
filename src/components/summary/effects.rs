use std::collections::HashSet;

use leptos::{html, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{icon::Icon, toggle_button::ToggleButton},
    effective::EffectiveCharacter,
    expr::Expr,
    model::{ActiveEffect, Attribute, Character},
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

    let name_input: NodeRef<html::Input> = NodeRef::new();
    let expr_input: NodeRef<html::Input> = NodeRef::new();
    let desc_input: NodeRef<html::Textarea> = NodeRef::new();

    view! {
        <div class="summary-section summary-section-effects">
            <h3 class="summary-section-title">{move_tr!("summary-effects")}</h3>

            // -- Add effect form --
            <div class="summary-list-entry">
                <div class="summary-list-row">
                    <button class="btn-icon btn-icon--success"
                        title=move_tr!("effect-add")
                        on:click=move |_| {
                            let Some(name_el) = name_input.get() else { return };
                            let Some(expr_el) = expr_input.get() else { return };
                            let Some(desc_el) = desc_input.get() else { return };

                            let name = name_el.value().trim().to_string();
                            if name.is_empty() { return; }

                            let Ok(expr) = parse_expr(&expr_el.value()) else {
                                return;
                            };

                            let description = desc_el.value().trim().to_string();

                            effects.update(|e| e.add(ActiveEffect {
                                name,
                                description,
                                expr,
                                enabled: true,
                            }, &store.read()));

                            name_el.set_value("");
                            expr_el.set_value("");
                            desc_el.set_value("");
                        }
                    ><Icon name="circle-plus" size=14 /></button>
                    <input type="text" class="summary-list-name" placeholder=move_tr!("effect-name") node_ref=name_input />
                    <input type="text" class="summary-list-name" placeholder=move_tr!("effect-expr") node_ref=expr_input />
                </div>
                <textarea class="summary-item-desc" placeholder=move_tr!("description") node_ref=desc_input />
            </div>

            // -- Effect list --
            {
                let expanded = RwSignal::new(HashSet::<usize>::new());
                move || {
                let effects_data = effects.read();
                let effect_list = effects_data.effects();
                if effect_list.is_empty() {
                    return None;
                }
                Some(view! {
                    <div class="summary-list">
                        {effect_list.iter().enumerate().map(|(i, effect)| {
                            let name = effect.name.clone();
                            let expr_str = effect.expr.as_ref().map(|expr| format!("{expr}")).unwrap_or_default();
                            let description = effect.description.clone();
                            let enabled = effect.enabled;
                            let is_open = Signal::derive(move || expanded.get().contains(&i));
                            view! {
                                <div class="summary-list-entry" class:disabled=!enabled>
                                    <div class="summary-list-row">
                                        <ToggleButton
                                            expanded=is_open
                                            on_toggle=move || expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                        />
                                        <label class="spell-prepared">
                                            <input
                                                type="checkbox"
                                                prop:checked=enabled
                                                on:change=move |_| effects.update(|e| e.toggle(i, &store.read()))
                                            />
                                        </label>
                                        <input
                                            type="text"
                                            class="summary-list-name"
                                            prop:value=name
                                            on:change=move |ev| {
                                                let new_name = event_target_value(&ev).trim().to_string();
                                                if new_name.is_empty() { return; }
                                                effects.update(|e| e.update_field(i, |eff| eff.name = new_name));
                                            }
                                        />
                                        <input
                                            type="text"
                                            class="summary-list-name"
                                            prop:value=expr_str
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
                                        <button
                                            class="btn-icon btn-icon--danger"
                                            on:click=move |_| { effects.update(|e| { e.remove(i, &store.read()); }); }
                                        >
                                            <Icon name="circle-minus" size=14 />
                                        </button>
                                    </div>
                                    <Show when=move || is_open.get()>
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
        </div>
    }
}
