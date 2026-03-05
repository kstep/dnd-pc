use std::collections::HashSet;

use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{icon::Icon, panel::Panel, toggle_button::ToggleButton},
    model::{
        Armor, ArmorType, Character, CharacterStoreFields, CurrencyStoreFields, DamageType,
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
                    <Icon name="arrow-down-a-z" />
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
                            let atk = weapon.attack_bonus.to_string();
                            let dmg = weapon.damage.clone();
                            let dmg_type = weapon.damage_type.map(|dt| dt as u8);
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
                                        type="number"
                                        placeholder=move_tr!("atk-bonus")
                                        class="short-input"
                                        prop:value=atk
                                        on:input=move |e| {
                                            weapons.write()[i].attack_bonus = event_target_value(&e).parse().unwrap_or(0);
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
                                    <select
                                        class="short-input"
                                        prop:value=dmg_type.map(|dt| dt.to_string()).unwrap_or_default()
                                        on:change=move |e| {
                                            let value = event_target_value(&e);
                                            weapons.write()[i].damage_type = if value.is_empty() {
                                                None
                                            } else {
                                                serde_json::from_str::<DamageType>(&value).ok()
                                            };
                                        }
                                    >
                                        <option value="" selected=dmg_type.is_none()>"\u{2014}"</option>
                                        {DamageType::iter()
                                            .map(|dt| {
                                                let option_value = (dt as u8).to_string();
                                                let selected = dmg_type == Some(dt as u8);
                                                let label = Signal::derive(move || i18n.tr(dt.tr_key()));
                                                view! {
                                                    <option value=option_value selected=selected>
                                                        {label}
                                                    </option>
                                                }
                                            })
                                            .collect_view()}
                                    </select>
                                    <button
                                        class="btn-remove"
                                        on:click=move |_| {
                                            if i < weapons.read().len() {
                                                weapons.write().remove(i);
                                            }
                                        }
                                    >
                                        <Icon name="x" size=14 />
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
                    <Icon name="arrow-down-a-z" />
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
                                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                armors.write()[i].base_ac = value;
                                            }
                                        }
                                    />
                                    <select
                                        prop:value=armor_type.to_string()
                                        on:change=move |e| {
                                            let value = event_target_value(&e);
                                            if let Ok(idx) = value.parse::<u8>() {
                                                let armor_type = match idx {
                                                    1 => ArmorType::Medium,
                                                    2 => ArmorType::Heavy,
                                                    _ => ArmorType::Light,
                                                };
                                                armors.write()[i].armor_type = armor_type;
                                            }
                                        }
                                    >
                                        {ArmorType::iter()
                                            .map(|at| {
                                                let option_value = (at as u8).to_string();
                                                let selected = at as u8 == armor_type;
                                                let label = Signal::derive(move || i18n.tr(at.tr_key()));
                                                view! {
                                                    <option value=option_value selected=selected>
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
                                        <Icon name="x" size=14 />
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
                    <Icon name="arrow-down-a-z" />
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
                                                    if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                                        items.write()[i].quantity = value;
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
                                                <Icon name="x" size=14 />
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
                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                currency.cp().set(value);
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
                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                currency.sp().set(value);
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
                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                currency.ep().set(value);
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
                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                currency.gp().set(value);
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
                            if let Ok(value) = event_target_value(&e).parse::<u32>() {
                                currency.pp().set(value);
                            }
                        }
                    />
                </div>
            </div>
        </Panel>
    }
}
