use leptos::prelude::*;

use crate::model::{Character, Item, Weapon};

#[component]
pub fn EquipmentPanel() -> impl IntoView {
    let char_signal = expect_context::<RwSignal<Character>>();

    let weapons = Memo::new(move |_| char_signal.get().equipment.weapons.clone());
    let items = Memo::new(move |_| char_signal.get().equipment.items.clone());
    let cp = Memo::new(move |_| char_signal.get().equipment.currency.cp);
    let sp = Memo::new(move |_| char_signal.get().equipment.currency.sp);
    let ep = Memo::new(move |_| char_signal.get().equipment.currency.ep);
    let gp = Memo::new(move |_| char_signal.get().equipment.currency.gp);
    let pp = Memo::new(move |_| char_signal.get().equipment.currency.pp);

    view! {
        <div class="panel equipment-panel">
            <h3>"Equipment"</h3>

            <h4>"Weapons"</h4>
            <div class="weapons-list">
                {move || {
                    weapons
                        .get()
                        .into_iter()
                        .enumerate()
                        .map(|(i, weapon)| {
                            view! {
                                <div class="weapon-entry">
                                    <input
                                        type="text"
                                        placeholder="Name"
                                        prop:value=weapon.name.clone()
                                        on:input=move |e| {
                                            char_signal.update(|c| {
                                                if let Some(w) = c.equipment.weapons.get_mut(i) {
                                                    w.name = event_target_value(&e);
                                                }
                                            });
                                        }
                                    />
                                    <input
                                        type="text"
                                        placeholder="Atk Bonus"
                                        class="short-input"
                                        prop:value=weapon.attack_bonus.clone()
                                        on:input=move |e| {
                                            char_signal.update(|c| {
                                                if let Some(w) = c.equipment.weapons.get_mut(i) {
                                                    w.attack_bonus = event_target_value(&e);
                                                }
                                            });
                                        }
                                    />
                                    <input
                                        type="text"
                                        placeholder="Damage"
                                        prop:value=weapon.damage.clone()
                                        on:input=move |e| {
                                            char_signal.update(|c| {
                                                if let Some(w) = c.equipment.weapons.get_mut(i) {
                                                    w.damage = event_target_value(&e);
                                                }
                                            });
                                        }
                                    />
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            char_signal.update(|c| {
                                                if i < c.equipment.weapons.len() {
                                                    c.equipment.weapons.remove(i);
                                                }
                                            });
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
                    char_signal.update(|c| c.equipment.weapons.push(Weapon::default()));
                }
            >
                "+ Add Weapon"
            </button>

            <h4>"Items"</h4>
            <div class="items-list">
                {move || {
                    items
                        .get()
                        .into_iter()
                        .enumerate()
                        .map(|(i, item)| {
                            view! {
                                <div class="item-entry">
                                    <input
                                        type="text"
                                        placeholder="Item name"
                                        prop:value=item.name.clone()
                                        on:input=move |e| {
                                            char_signal.update(|c| {
                                                if let Some(it) = c.equipment.items.get_mut(i) {
                                                    it.name = event_target_value(&e);
                                                }
                                            });
                                        }
                                    />
                                    <input
                                        type="number"
                                        class="short-input"
                                        placeholder="Qty"
                                        min="0"
                                        prop:value=item.quantity.to_string()
                                        on:input=move |e| {
                                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                char_signal.update(|c| {
                                                    if let Some(it) = c.equipment.items.get_mut(i) {
                                                        it.quantity = v;
                                                    }
                                                });
                                            }
                                        }
                                    />
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            char_signal.update(|c| {
                                                if i < c.equipment.items.len() {
                                                    c.equipment.items.remove(i);
                                                }
                                            });
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
                    char_signal.update(|c| c.equipment.items.push(Item { quantity: 1, ..Item::default() }));
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
                        prop:value=move || cp.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                char_signal.update(|c| c.equipment.currency.cp = v);
                            }
                        }
                    />
                </div>
                <div class="currency-field">
                    <label>"SP"</label>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || sp.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                char_signal.update(|c| c.equipment.currency.sp = v);
                            }
                        }
                    />
                </div>
                <div class="currency-field">
                    <label>"EP"</label>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || ep.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                char_signal.update(|c| c.equipment.currency.ep = v);
                            }
                        }
                    />
                </div>
                <div class="currency-field">
                    <label>"GP"</label>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || gp.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                char_signal.update(|c| c.equipment.currency.gp = v);
                            }
                        }
                    />
                </div>
                <div class="currency-field">
                    <label>"PP"</label>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || pp.get().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                char_signal.update(|c| c.equipment.currency.pp = v);
                            }
                        }
                    />
                </div>
            </div>
        </div>
    }
}
