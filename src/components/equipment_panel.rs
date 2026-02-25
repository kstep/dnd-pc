use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{panel::Panel, toggle_button::ToggleButton},
    model::{
        Character, CharacterStoreFields, CurrencyStoreFields, EquipmentStoreFields, Item, Weapon,
    },
};

#[component]
pub fn EquipmentPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let equipment = store.equipment();
    let weapons = equipment.weapons();
    let items = equipment.items();
    let currency = equipment.currency();

    view! {
        <Panel title=move_tr!("panel-equipment") class="equipment-panel">

            <h4>{move_tr!("weapons")}</h4>
            <div class="weapons-list">
                {move || {
                    weapons
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, weapon)| {
                            let name = weapon.name.clone();
                            let atk = weapon.attack_bonus.clone();
                            let dmg = weapon.damage.clone();
                            let dmg_type = weapon.damage_type.clone();
                            view! {
                                <div class="weapon-entry">
                                    <input
                                        type="text"
                                        placeholder=move_tr!("name")
                                        prop:value=name
                                        on:input=move |e| {
                                            weapons.write()[i].name = event_target_value(&e);
                                        }
                                    />
                                    <input
                                        type="text"
                                        placeholder=move_tr!("atk-bonus")
                                        class="short-input"
                                        prop:value=atk
                                        on:input=move |e| {
                                            weapons.write()[i].attack_bonus = event_target_value(&e);
                                        }
                                    />
                                    <input
                                        type="text"
                                        placeholder=move_tr!("damage")
                                        prop:value=dmg
                                        on:input=move |e| {
                                            weapons.write()[i].damage = event_target_value(&e);
                                        }
                                    />
                                    <input
                                        type="text"
                                        placeholder=move_tr!("type")
                                        class="short-input"
                                        prop:value=dmg_type
                                        on:input=move |e| {
                                            weapons.write()[i].damage_type = event_target_value(&e);
                                        }
                                    />
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            if i < weapons.read().len() {
                                                weapons.write().remove(i);
                                            }
                                        }
                                    >
                                        "X"
                                    </button>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button
                class="btn-add"
                on:click=move |_| {
                    weapons.write().push(Weapon::default());
                }
            >
                {move_tr!("btn-add-weapon")}
            </button>

            <h4>{move_tr!("items")}</h4>
            {
                let items_expanded = RwSignal::new(HashSet::<usize>::new());
                view! {
                    <div class="items-list">
                        {move || {
                            items
                                .read()
                                .iter()
                                .enumerate()
                                .map(|(i, item)| {
                                    let name = item.name.clone();
                                    let qty = item.quantity.to_string();
                                    let desc = item.description.clone();
                                    let is_open = Signal::derive(move || items_expanded.get().contains(&i));
                                    view! {
                                        <div class="item-entry">
                                            <ToggleButton
                                                expanded=is_open
                                                on_toggle=move || items_expanded.update(|set| { if !set.remove(&i) { set.insert(i); } })
                                            />
                                            <input
                                                type="text"
                                                placeholder=move_tr!("item-name")
                                                prop:value=name
                                                on:input=move |e| {
                                                    items.write()[i].name = event_target_value(&e);
                                                }
                                            />
                                            <input
                                                type="number"
                                                class="short-input"
                                                placeholder=move_tr!("qty")
                                                min="0"
                                                prop:value=qty
                                                on:input=move |e| {
                                                    if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                        items.write()[i].quantity = v;
                                                    }
                                                }
                                            />
                                            <button
                                                class="btn-remove"
                                                on:click=move |_| {
                                                    if i < items.read().len() {
                                                        items.write().remove(i);
                                                    }
                                                }
                                            >
                                                "X"
                                            </button>
                                            <Show when=move || is_open.get()>
                                                <textarea
                                                    class="item-desc"
                                                    placeholder=move_tr!("description")
                                                    prop:value=desc.clone()
                                                    on:change=move |e| {
                                                        items.write()[i].description = event_target_value(&e);
                                                    }
                                                />
                                            </Show>
                                        </div>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                }
            }
            <button
                class="btn-add"
                on:click=move |_| {
                    items.write().push(Item { quantity: 1, ..Item::default() });
                }
            >
                {move_tr!("btn-add-item")}
            </button>

            <h4>{move_tr!("currency")}</h4>
            <div class="currency-row">
                // CP: only > (break 1 SP into 10 CP)
                <div class="currency-field">
                    <div class="currency-label">
                        <span class="btn-convert-placeholder" />
                        <label>"CP"</label>
                        <button class="btn-convert" on:click=move |_| {
                            if currency.sp().get_untracked() >= 1 {
                                currency.sp().update(|v| *v -= 1);
                                currency.cp().update(|v| *v += 10);
                            }
                        }>">"</button>
                    </div>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || currency.cp().get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                currency.cp().set(v);
                            }
                        }
                    />
                </div>
                // SP: < (break 1 SP into 10 CP), > (break 1 EP into 5 SP)
                <div class="currency-field">
                    <div class="currency-label">
                        <button class="btn-convert" on:click=move |_| {
                            if currency.sp().get_untracked() >= 1 {
                                currency.sp().update(|v| *v -= 1);
                                currency.cp().update(|v| *v += 10);
                            }
                        }>"<"</button>
                        <label>"SP"</label>
                        <button class="btn-convert" on:click=move |_| {
                            if currency.ep().get_untracked() >= 1 {
                                currency.ep().update(|v| *v -= 1);
                                currency.sp().update(|v| *v += 5);
                            }
                        }>">"</button>
                    </div>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || currency.sp().get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                currency.sp().set(v);
                            }
                        }
                    />
                </div>
                // EP: < (break 1 EP into 5 SP), > (break 1 GP into 5 EP)
                <div class="currency-field">
                    <div class="currency-label">
                        <button class="btn-convert" on:click=move |_| {
                            if currency.ep().get_untracked() >= 1 {
                                currency.ep().update(|v| *v -= 1);
                                currency.sp().update(|v| *v += 5);
                            }
                        }>"<"</button>
                        <label>"EP"</label>
                        <button class="btn-convert" on:click=move |_| {
                            if currency.gp().get_untracked() >= 1 {
                                currency.gp().update(|v| *v -= 1);
                                currency.ep().update(|v| *v += 5);
                            }
                        }>">"</button>
                    </div>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || currency.ep().get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                currency.ep().set(v);
                            }
                        }
                    />
                </div>
                // GP: < (break 1 GP into 5 EP), > (break 1 PP into 10 GP)
                <div class="currency-field">
                    <div class="currency-label">
                        <button class="btn-convert" on:click=move |_| {
                            if currency.gp().get_untracked() >= 1 {
                                currency.gp().update(|v| *v -= 1);
                                currency.ep().update(|v| *v += 5);
                            }
                        }>"<"</button>
                        <label>"GP"</label>
                        <button class="btn-convert" on:click=move |_| {
                            if currency.pp().get_untracked() >= 1 {
                                currency.pp().update(|v| *v -= 1);
                                currency.gp().update(|v| *v += 10);
                            }
                        }>">"</button>
                    </div>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || currency.gp().get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                currency.gp().set(v);
                            }
                        }
                    />
                </div>
                // PP: only < (break 1 PP into 10 GP)
                <div class="currency-field">
                    <div class="currency-label">
                        <button class="btn-convert" on:click=move |_| {
                            if currency.pp().get_untracked() >= 1 {
                                currency.pp().update(|v| *v -= 1);
                                currency.gp().update(|v| *v += 10);
                            }
                        }>"<"</button>
                        <label>"PP"</label>
                        <span class="btn-convert-placeholder" />
                    </div>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || currency.pp().get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                currency.pp().set(v);
                            }
                        }
                    />
                </div>
            </div>
        </Panel>
    }
}
