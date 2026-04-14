use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;
use strum::IntoEnumIterator;

use crate::{
    components::{icon::Icon, slot_box::SlotBox, toggle_button::ToggleButton},
    model::{
        Armor, ArmorType, Character, CharacterStoreFields, CurrencyStoreFields, DamageType,
        EquipmentStoreFields, Item, Translatable, Weapon, WeaponEffect,
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

    let currency_expanded = RwSignal::new(false);
    let show_cp = move || currency_expanded.get() || currency.cp().get() > 0;
    let show_sp = move || currency_expanded.get() || currency.sp().get() > 0;
    let show_ep = move || currency_expanded.get() || currency.ep().get() > 0;
    let show_pp = move || currency_expanded.get() || currency.pp().get() > 0;

    view! {
        <section>
            <div class="section-header">
                <button
                    class="btn-toggle-desc"
                    class:expanded=move || currency_expanded.get()
                    on:click=move |_| currency_expanded.update(|v| *v = !*v)
                />
                <h3>{move_tr!("currency")}</h3>
            </div>
            <div class="currency-row">
                <Show when=show_pp>
                    <SlotBox label="PP" label_after=true>
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
                        <button class="btn-convert" title="→ GP" on:click=move |_| {
                            if currency.pp().get_untracked() >= 1 {
                                currency.pp().update(|v| *v -= 1);
                                currency.gp().update(|v| *v += 10);
                            }
                        }>">"</button>
                    </SlotBox>
                </Show>
                <SlotBox label="GP" label_after=true>
                    <button class="btn-convert" title="→ PP" on:click=move |_| {
                        if currency.gp().get_untracked() >= 10 {
                            currency.gp().update(|v| *v -= 10);
                            currency.pp().update(|v| *v += 1);
                        }
                    }>"<"</button>
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
                    <button class="btn-convert" title="→ EP" on:click=move |_| {
                        if currency.gp().get_untracked() >= 1 {
                            currency.gp().update(|v| *v -= 1);
                            currency.ep().update(|v| *v += 5);
                        }
                    }>">"</button>
                </SlotBox>
                <Show when=show_ep>
                    <SlotBox label="EP" label_after=true>
                        <button class="btn-convert" title="→ GP" on:click=move |_| {
                            if currency.ep().get_untracked() >= 5 {
                                currency.ep().update(|v| *v -= 5);
                                currency.gp().update(|v| *v += 1);
                            }
                        }>"<"</button>
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
                        <button class="btn-convert" title="→ SP" on:click=move |_| {
                            if currency.ep().get_untracked() >= 1 {
                                currency.ep().update(|v| *v -= 1);
                                currency.sp().update(|v| *v += 5);
                            }
                        }>">"</button>
                    </SlotBox>
                </Show>
                <Show when=show_sp>
                    <SlotBox label="SP" label_after=true>
                        <button class="btn-convert" title="→ EP" on:click=move |_| {
                            if currency.sp().get_untracked() >= 5 {
                                currency.sp().update(|v| *v -= 5);
                                currency.ep().update(|v| *v += 1);
                            }
                        }>"<"</button>
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
                        <button class="btn-convert" title="→ CP" on:click=move |_| {
                            if currency.sp().get_untracked() >= 1 {
                                currency.sp().update(|v| *v -= 1);
                                currency.cp().update(|v| *v += 10);
                            }
                        }>">"</button>
                    </SlotBox>
                </Show>
                <Show when=show_cp>
                    <SlotBox label="CP" label_after=true>
                        <button class="btn-convert" title="→ SP" on:click=move |_| {
                            if currency.cp().get_untracked() >= 10 {
                                currency.cp().update(|v| *v -= 10);
                                currency.sp().update(|v| *v += 1);
                            }
                        }>"<"</button>
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
                    </SlotBox>
                </Show>
            </div>
        </section>

        <section>
            <div class="section-header">
                <h3>{move_tr!("weapons")}</h3>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        weapons.write().sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    }
                >
                    <Icon name="arrow-down-a-z" size=16 />
                </button>
            </div>
            {
                view! {
                    <div class="entry-list">
                        {move || {
                            weapons
                                .read()
                                .iter()
                                .enumerate()
                                .map(|(i, weapon)| {
                                    let name = weapon.name.clone();
                                    let atk = weapon.attack_bonus.to_string();
                                    let effects: Vec<_> = weapon.effects.iter().enumerate().map(|(j, effect)| {
                                        let eff_name = effect.name.clone();
                                        let eff_expr = effect.expr.to_string();
                                        let eff_dmg_type = effect.damage_type.map(|dt| dt as u8);
                                        (j, eff_name, eff_expr, eff_dmg_type)
                                    }).collect();
                                    view! {
                                        <div class="entry-item">
                                            <ToggleButton />
                                            <div class="entry-content">
                                                <input
                                                    type="text"
                                                    class="entry-name"
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
                                            </div>
                                            <div class="entry-actions">
                                                <button
                                                    class="btn-remove"
                                                    on:click=move |_| {
                                                        if i < weapons.read().len() {
                                                            weapons.write().remove(i);
                                                        }
                                                    }
                                                >
                                                    <Icon name="x" />
                                                </button>
                                            </div>
                                            {effects.into_iter().map(|(j, eff_name, eff_expr, eff_dmg_type)| {
                                                view! {
                                                    <div class="entry-full-row weapon-effect-row">
                                                        <input
                                                            type="text"
                                                            placeholder=move_tr!("effect-name")
                                                            class="effect-name-input"
                                                            prop:value=eff_name
                                                            on:input=move |e| {
                                                                weapons.write()[i].effects[j].name = event_target_value(&e);
                                                            }
                                                        />
                                                        <select class="select-fixed"
                                                            prop:value=eff_dmg_type.map(|dt| dt.to_string()).unwrap_or_default()
                                                            on:change=move |e| {
                                                                let value = event_target_value(&e);
                                                                weapons.write()[i].effects[j].damage_type = if value.is_empty() {
                                                                    None
                                                                } else {
                                                                    DamageType::from_u8_str(&value)
                                                                };
                                                            }
                                                        >
                                                            <option value="" selected=eff_dmg_type.is_none()>"\u{2014}"</option>
                                                            {DamageType::iter()
                                                                .map(|dt| {
                                                                    let option_value = (dt as u8).to_string();
                                                                    let selected = eff_dmg_type == Some(dt as u8);
                                                                    let label = Signal::derive(move || i18n.tr(dt.tr_key()));
                                                                    view! {
                                                                        <option value=option_value selected=selected>
                                                                            {label}
                                                                        </option>
                                                                    }
                                                                })
                                                                .collect_view()}
                                                        </select>
                                                        <input
                                                            type="text"
                                                            placeholder=move_tr!("damage")
                                                            class="damage-input"
                                                            prop:value=eff_expr
                                                            on:change=move |e| {
                                                                let value = event_target_value(&e);
                                                                if let Ok(expr) = value.parse() {
                                                                    weapons.write()[i].effects[j].expr = expr;
                                                                }
                                                            }
                                                        />
                                                        <button
                                                            class="btn-remove"
                                                            on:click=move |_| {
                                                                if j < weapons.read()[i].effects.len() {
                                                                    weapons.write()[i].effects.remove(j);
                                                                }
                                                            }
                                                        >
                                                            <Icon name="x" />
                                                        </button>
                                                    </div>
                                                }
                                            }).collect_view()}
                                            <div class="entry-full-row">
                                                <button
                                                    class="btn-primary"
                                                    on:click=move |_| {
                                                        weapons.write()[i].effects.push(WeaponEffect::default());
                                                    }
                                                >
                                                    "+ " {move_tr!("btn-add-effect")}
                                                </button>
                                            </div>
                                        </div>
                            }
                        })
                        .collect_view()
                }}
                    </div>
                }
            }
            <button
                class="btn-primary"
                on:click=move |_| {
                    weapons.write().push(Weapon::default());
                }
            >
                {move_tr!("btn-add-weapon")}
            </button>
        </section>

        <section>
            <div class="section-header">
                <h3>{move_tr!("armor")}</h3>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        armors.write().sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    }
                >
                    <Icon name="arrow-down-a-z" size=16 />
                </button>
            </div>
            {
                view! {
                    <div class="entry-list">
                        {move || {
                            armors
                                .read()
                                .iter()
                                .enumerate()
                                .map(|(i, armor)| {
                                    let name = armor.name.clone();
                                    let base_ac = armor.base_ac.to_string();
                                    let armor_type = armor.armor_type as u8;
                                    let is_natural = armor.armor_type == ArmorType::Natural;
                                    let ac_expr_str = armor.ac_expr.as_ref().map(|e| e.to_string()).unwrap_or_default();
                                    if is_natural {
                                        Either::Left(view! {
                                            <div class="entry-item armor-natural">
                                                <ToggleButton />
                                                <div class="entry-content">
                                                    <span class="entry-name armor-name">{name}</span>
                                                    <span class="armor-type-label">
                                                        {move || i18n.tr(ArmorType::Natural.tr_key())}
                                                    </span>
                                                </div>
                                                <div class="entry-actions">
                                                    <button
                                                        class="btn-remove"
                                                        on:click=move |_| {
                                                            if i < armors.read().len() {
                                                                armors.write().remove(i);
                                                            }
                                                        }
                                                    >
                                                        <Icon name="x" />
                                                    </button>
                                                </div>
                                                <span class="entry-sublabel">{ac_expr_str.clone()}</span>
                                            </div>
                                        })
                                    } else {
                                        Either::Right(view! {
                                            <div class="entry-item">
                                                <ToggleButton />
                                                <div class="entry-content">
                                                    <input
                                                        type="text"
                                                        class="entry-name"
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
                                                                let mut armors = armors.write();
                                                                let old_base_ac = armors[i].base_ac;
                                                                let at = armors[i].armor_type;
                                                                armors[i].base_ac = value;
                                                                // Auto-fill formula if it matches the previous default
                                                                let old_default = Armor::default_ac_expr(at, old_base_ac);
                                                                if armors[i].ac_expr == old_default || armors[i].ac_expr.is_none() {
                                                                    armors[i].ac_expr = Armor::default_ac_expr(at, value);
                                                                }
                                                            }
                                                        }
                                                    />
                                                    <select class="select-fixed"
                                                        prop:value=armor_type.to_string()
                                                        on:change=move |e| {
                                                            let value = event_target_value(&e);
                                                            if let Ok(idx) = value.parse::<u8>() {
                                                                let new_type = ArmorType::try_from(idx).unwrap_or_default();
                                                                let mut armors = armors.write();
                                                                let old_type = armors[i].armor_type;
                                                                let base_ac = armors[i].base_ac;
                                                                armors[i].armor_type = new_type;
                                                                // Auto-fill formula if it matches the previous default
                                                                let old_default = Armor::default_ac_expr(old_type, base_ac);
                                                                if armors[i].ac_expr == old_default || armors[i].ac_expr.is_none() {
                                                                    armors[i].ac_expr = Armor::default_ac_expr(new_type, base_ac);
                                                                }
                                                            }
                                                        }
                                                    >
                                                        {ArmorType::iter()
                                                            .filter(|at| *at != ArmorType::Natural)
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
                                                </div>
                                                <div class="entry-actions">
                                                    <button
                                                        class="btn-remove"
                                                        on:click=move |_| {
                                                            if i < armors.read().len() {
                                                                armors.write().remove(i);
                                                            }
                                                        }
                                                    >
                                                        <Icon name="x" />
                                                    </button>
                                                </div>
                                                <input
                                                    type="text"
                                                    placeholder=move_tr!("ac-formula")
                                                    class="entry-desc ac-expr-input"
                                                    prop:value=ac_expr_str.clone()
                                                    on:change=move |e| {
                                                        let value = event_target_value(&e);
                                                        armors.write()[i].ac_expr = if value.is_empty() {
                                                            None
                                                        } else {
                                                            value.parse().ok()
                                                        };
                                                    }
                                                />
                                            </div>
                                        })
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                }
            }
            <button
                class="btn-primary"
                on:click=move |_| {
                    armors.write().push(Armor::default());
                }
            >
                {move_tr!("btn-add-armor")}
            </button>
        </section>

        <section>
            <div class="section-header">
                <h3>{move_tr!("items")}</h3>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        items.write().sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    }
                >
                    <Icon name="arrow-down-a-z" size=16 />
                </button>
            </div>
            {
                view! {
                    <div class="entry-list">
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
                                        <div class="entry-item">
                                            <ToggleButton />
                                            <div class="entry-content">
                                                <input
                                                    type="text"
                                                    class="entry-name"
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
                                            </div>
                                            <div class="entry-actions">
                                                <button
                                                    class="btn-remove"
                                                    on:click=move |_| {
                                                        if i < items.read().len() {
                                                            items.write().remove(i);
                                                        }
                                                    }
                                                >
                                                    <Icon name="x" />
                                                </button>
                                            </div>
                                            <textarea
                                                class="entry-desc"
                                                placeholder=move_tr!("description")
                                                prop:value=desc.clone()
                                                on:change=move |e| {
                                                    items.write()[i].description = event_target_value(&e);
                                                }
                                            />
                                        </div>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                }
            }
            <button
                class="btn-primary"
                on:click=move |_| {
                    items.write().push(Item { quantity: 1, ..Item::default() });
                }
            >
                {move_tr!("btn-add-item")}
            </button>
        </section>
    }
}
