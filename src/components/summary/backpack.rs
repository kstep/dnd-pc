use std::collections::HashSet;

use leptos::{either::Either, html, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{icon::Icon, toggle_button::ToggleButton},
    model::{Character, CharacterStoreFields, EquipmentStoreFields, Item, Money},
};

#[component]
pub fn BackpackBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let equipment = store.equipment();
    let money_input: NodeRef<html::Input> = NodeRef::new();

    let name_input: NodeRef<html::Input> = NodeRef::new();
    let qty_input: NodeRef<html::Input> = NodeRef::new();
    let desc_input: NodeRef<html::Textarea> = NodeRef::new();

    let money_value = move || {
        money_input.read().as_ref().and_then(|input| {
            let value = Money::from_gp_str(&input.value())?;
            input.set_value("");
            Some(value)
        })
    };

    view! {
        <div class="summary-section">
            <h3 class="summary-section-title">{move_tr!("summary-backpack")}</h3>

            // -- Currency --
            <div class="summary-currency">
                <label>{move_tr!("currency")}</label>
                <span>{move || equipment.currency().read().to_string()}</span>
                <div class="summary-currency-controls">
                    <input type="text" required inputmode="decimal" class="summary-currency-input" node_ref=money_input />
                    <span class="summary-currency-unit">"gp"</span>
                    <div class="btn-container">
                        <button class="btn-icon btn-icon--danger" title=move_tr!("spend")
                            on:click=move |_| {
                                if let Some(amount) = money_value() {
                                    equipment.currency().update(|c| { c.spend(amount); });
                                }
                            }
                        ><Icon name="circle-minus" size=14 /></button>
                        <button class="btn-icon btn-icon--success" title=move_tr!("gain")
                            on:click=move |_| {
                                if let Some(amount) = money_value() {
                                    equipment.currency().update(|c| c.gain(amount));
                                }
                            }
                        ><Icon name="circle-plus" size=14 /></button>
                    </div>
                </div>
            </div>

            // -- Add item --
            <div class="entry-item">
                <button class="btn-icon btn-icon--success" title=move_tr!("add-item")
                    on:click=move |_| {
                        let Some(name_el) = name_input.get() else { return };
                        let Some(qty_el) = qty_input.get() else { return };
                        let Some(desc_input) = desc_input.get() else { return };

                        let name = name_el.value().trim().to_string();
                        if name.is_empty() { return; }

                        let quantity: u32 = qty_el
                            .value()
                            .parse()
                            .unwrap_or(1);
                        if quantity == 0 { return; }

                        let description = desc_input.value().trim().to_string();

                        equipment.items().write().push(Item {
                            name,
                            quantity,
                            description,
                        });

                        name_el.set_value("");
                        qty_el.set_value("1");
                        desc_input.set_value("");
                    }
                ><Icon name="circle-plus" size=14 /></button>
                <div class="entry-content">
                    <input type="text" required class="entry-name" placeholder=move_tr!("item-name") node_ref=name_input />
                    <span class="entry-badge">
                        "\u{00d7}"
                        <input type="number" class="summary-qty-input" min="1" required value="1" node_ref=qty_input />
                    </span>
                </div>
                <div class="entry-actions" />
                <textarea class="entry-desc" placeholder=move_tr!("description") node_ref=desc_input />
            </div>

            {move || {
                let items = equipment.items().read().iter()
                    .enumerate()
                    .filter(|(_, item)| !item.name.is_empty())
                    .map(|(idx, item)| (idx, item.name.clone(), item.quantity, item.description.clone()))
                    .collect::<Vec<_>>();
                if items.is_empty() {
                    Either::Left(view! {
                        <p class="summary-empty">{move_tr!("summary-no-items")}</p>
                    })
                } else {
                    let expanded = RwSignal::new(HashSet::<usize>::new());
                    Either::Right(view! {
                        <div class="entry-list">
                            {items.into_iter().map(|(idx, name, qty, desc)| {
                                let is_open = Signal::derive(move || expanded.get().contains(&idx));
                                view! {
                                    <div class="entry-item">
                                        <ToggleButton
                                            expanded=is_open
                                            on_toggle=move || expanded.update(|set| { if !set.remove(&idx) { set.insert(idx); } })
                                        />
                                        <div class="entry-content">
                                            <span class="entry-name">{name}</span>
                                            <span class="entry-badge">
                                                "\u{00d7}"
                                                <input
                                                    type="number"
                                                    class="summary-qty-input"
                                                    min="0"
                                                    prop:value=qty.to_string()
                                                    on:input=move |e| {
                                                        if let Ok(value) = event_target_value(&e).parse() {
                                                            equipment.items().write()[idx].quantity = value;
                                                        }
                                                    }
                                                />
                                            </span>
                                        </div>
                                        <div class="entry-actions" />
                                        <Show when=move || is_open.get()>
                                            <textarea
                                                class="entry-desc"
                                                prop:value=desc.clone()
                                                on:input=move |e| {
                                                    let value = event_target_value(&e);
                                                    equipment.items().write()[idx].description = value;
                                                }
                                            />
                                        </Show>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    })
                }
            }}

        </div>
    }
}
