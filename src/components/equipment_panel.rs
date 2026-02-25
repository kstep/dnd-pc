use leptos::prelude::*;
use reactive_stores::Store;

use crate::{
    components::panel::Panel,
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
        <Panel title="Equipment" class="equipment-panel">

            <h4>"Weapons"</h4>
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
                                        placeholder="Name"
                                        prop:value=name
                                        on:input=move |e| {
                                            weapons.write()[i].name = event_target_value(&e);
                                        }
                                    />
                                    <input
                                        type="text"
                                        placeholder="Atk Bonus"
                                        class="short-input"
                                        prop:value=atk
                                        on:input=move |e| {
                                            weapons.write()[i].attack_bonus = event_target_value(&e);
                                        }
                                    />
                                    <input
                                        type="text"
                                        placeholder="Damage"
                                        prop:value=dmg
                                        on:input=move |e| {
                                            weapons.write()[i].damage = event_target_value(&e);
                                        }
                                    />
                                    <input
                                        type="text"
                                        placeholder="Type"
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
                "+ Add Weapon"
            </button>

            <h4>"Items"</h4>
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
                            view! {
                                <div class="item-entry">
                                    <input
                                        type="text"
                                        placeholder="Item name"
                                        prop:value=name
                                        on:input=move |e| {
                                            items.write()[i].name = event_target_value(&e);
                                        }
                                    />
                                    <input
                                        type="number"
                                        class="short-input"
                                        placeholder="Qty"
                                        min="0"
                                        prop:value=qty
                                        on:input=move |e| {
                                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                items.write()[i].quantity = v;
                                            }
                                        }
                                    />
                                    <input
                                        type="text"
                                        placeholder="Description"
                                        prop:value=desc
                                        on:input=move |e| {
                                            items.write()[i].description = event_target_value(&e);
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
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
            <button
                class="btn-add"
                on:click=move |_| {
                    items.write().push(Item { quantity: 1, ..Item::default() });
                }
            >
                "+ Add Item"
            </button>

            <h4>"Currency"</h4>
            <div class="currency-row">
                <div class="currency-field">
                    <label>"CP"</label>
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
                <div class="currency-field">
                    <label>"SP"</label>
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
                <div class="currency-field">
                    <label>"EP"</label>
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
                <div class="currency-field">
                    <label>"GP"</label>
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
                <div class="currency-field">
                    <label>"PP"</label>
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
