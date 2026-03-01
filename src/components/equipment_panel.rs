use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{panel::Panel, toggle_button::ToggleButton},
    model::{
        Armor, ArmorType, Character, CharacterStoreFields, CurrencyStoreFields,
        EquipmentStoreFields, Item, Translatable, Weapon,
    },
};

#[component]
pub fn EquipmentPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let i18n = expect_context::<leptos_fluent::I18n>();

    let equipment = store.equipment();
    let weapons = equipment.weapons();
    let armors = equipment.armors();
    let items = equipment.items();
    let currency = equipment.currency();

    view! {
        <Panel title=move_tr!("panel-equipment") class="equipment-panel">

            <div class="section-header">
                <h4>{move_tr!("weapons")}</h4>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        weapons.write().sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    }
                >
                    "\u{21C5}"
                </button>
            </div>
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

            <div class="section-header">
                <h4>{move_tr!("armor")}</h4>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        armors.write().sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    }
                >
                    "\u{21C5}"
                </button>
            </div>
            <div class="armors-list">
                {move || {
                    armors
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, armor)| {
                            let name = armor.name.clone();
                            let base_ac = armor.base_ac.to_string();
                            let armor_type = armor.armor_type as u8;
                            view! {
                                <div class="armor-entry">
                                    <input
                                        type="text"
                                        placeholder=move_tr!("name")
                                        prop:value=name
                                        on:input=move |e| {
                                            armors.write()[i].name = event_target_value(&e);
                                        }
                                    />
                                    <input
                                        type="number"
                                        placeholder=move_tr!("base-ac")
                                        class="short-input"
                                        min="0"
                                        prop:value=base_ac
                                        on:input=move |e| {
                                            if let Ok(v) = event_target_value(&e).parse::<u32>() {
                                                armors.write()[i].base_ac = v;
                                            }
                                        }
                                    />
                                    <select
                                        prop:value=armor_type.to_string()
                                        on:change=move |e| {
                                            let val = event_target_value(&e);
                                            if let Ok(idx) = val.parse::<u8>() {
                                                let at = match idx {
                                                    1 => ArmorType::Medium,
                                                    2 => ArmorType::Heavy,
                                                    _ => ArmorType::Light,
                                                };
                                                armors.write()[i].armor_type = at;
                                            }
                                        }
                                    >
                                        {ArmorType::iter()
                                            .map(|at| {
                                                let val = (at as u8).to_string();
                                                let selected = at as u8 == armor_type;
                                                let label = Signal::derive(move || i18n.tr(at.tr_key()));
                                                view! {
                                                    <option value=val selected=selected>
                                                        {label}
                                                    </option>
                                                }
                                            })
                                            .collect_view()}
                                    </select>
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            if i < armors.read().len() {
                                                armors.write().remove(i);
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
                    armors.write().push(Armor::default());
                }
            >
                {move_tr!("btn-add-armor")}
            </button>

            <div class="section-header">
                <h4>{move_tr!("items")}</h4>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        items.write().sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    }
                >
                    "\u{21C5}"
                </button>
            </div>
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
                // CP: only > (combine 10 CP into 1 SP)
                <div class="currency-field">
                    <div class="currency-label">
                        <span class="btn-convert-placeholder" />
                        <label>"CP"</label>
                        <button class="btn-convert" on:click=move |_| {
                            if currency.cp().get_untracked() >= 10 {
                                currency.cp().update(|v| *v -= 10);
                                currency.sp().update(|v| *v += 1);
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
                // SP: < (break 1 SP into 10 CP), > (combine 5 SP into 1 EP)
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
                            if currency.sp().get_untracked() >= 5 {
                                currency.sp().update(|v| *v -= 5);
                                currency.ep().update(|v| *v += 1);
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
                // EP: < (break 1 EP into 5 SP), > (combine 5 EP into 1 GP)
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
                            if currency.ep().get_untracked() >= 5 {
                                currency.ep().update(|v| *v -= 5);
                                currency.gp().update(|v| *v += 1);
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
                // GP: < (break 1 GP into 5 EP), > (combine 10 GP into 1 PP)
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
                            if currency.gp().get_untracked() >= 10 {
                                currency.gp().update(|v| *v -= 10);
                                currency.pp().update(|v| *v += 1);
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
