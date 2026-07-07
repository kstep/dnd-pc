use leptos::{either::Either, html, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        icon::Icon,
        session_list::{SessionList, SessionListItem},
    },
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
        <div class="session-section" id="session-backpack">
            <h3 class="session-section-title">{move_tr!("session-backpack")}</h3>

            // -- Currency --
            <div class="session-currency">
                <label>{move_tr!("currency")}</label>
                <span>{move || equipment.currency().read().to_string()}</span>
                <div class="session-currency-controls">
                    <input
                        type="text"
                        required
                        inputmode="decimal"
                        class="session-currency-input"
                        node_ref=money_input
                    />
                    <span class="session-currency-unit">"gp"</span>
                    <div class="btn-container">
                        <button
                            class="btn-icon btn-icon--danger"
                            title=move_tr!("spend")
                            on:click=move |_| {
                                if let Some(amount) = money_value() {
                                    equipment
                                        .currency()
                                        .update(|c| {
                                            c.spend(amount);
                                        });
                                }
                            }
                        >
                            <Icon name="circle-minus" />
                        </button>
                        <button
                            class="btn-icon btn-icon--success"
                            title=move_tr!("gain")
                            on:click=move |_| {
                                if let Some(amount) = money_value() {
                                    equipment.currency().update(|c| c.gain(amount));
                                }
                            }
                        >
                            <Icon name="circle-plus" />
                        </button>
                    </div>
                </div>
            </div>

            // -- Add item --
            <div class="entry-item">
                <button
                    class="btn-icon btn-icon--success"
                    title=move_tr!("add-item")
                    on:click=move |_| {
                        let Some(name_el) = name_input.get() else { return };
                        let Some(qty_el) = qty_input.get() else { return };
                        let Some(desc_input) = desc_input.get() else { return };
                        let name = name_el.value().trim().to_string();
                        if name.is_empty() {
                            return;
                        }
                        let quantity: u32 = qty_el.value().parse().unwrap_or(1);
                        if quantity == 0 {
                            return;
                        }
                        let description = desc_input.value().trim().to_string();
                        equipment
                            .items()
                            .write()
                            .push(Item {
                                name,
                                quantity,
                                description,
                                ..Item::default()
                            });
                        name_el.set_value("");
                        qty_el.set_value("1");
                        desc_input.set_value("");
                    }
                >
                    <Icon name="circle-plus" />
                </button>
                <div class="entry-content">
                    <input
                        type="text"
                        required
                        class="entry-name"
                        placeholder=move_tr!("item-name")
                        node_ref=name_input
                    />
                    <input
                        type="number"
                        class="entry-name session-qty-input"
                        min="1"
                        required
                        value="1"
                        node_ref=qty_input
                    />
                </div>
                <div class="entry-actions" />
                <textarea
                    class="entry-desc"
                    placeholder=move_tr!("description")
                    node_ref=desc_input
                />
            </div>

            {move || {
                let items_store = equipment.items();
                let items = items_store
                    .read()
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| !item.name.is_empty())
                    .map(|(idx, item)| {
                        let qty = item.quantity;
                        let desc = item.description.clone();
                        let equipped = item.equipped;
                        let attune_toggle = item
                            .requires_attunement
                            .then(|| {
                                view! {
                                    <button
                                        class="btn-icon attune-toggle"
                                        class:attuned=move || items_store.read()[idx].attuned
                                        title=move_tr!("attuned")
                                        on:click=move |_| {
                                            let attuned = items_store.read()[idx].attuned;
                                            items_store.write()[idx].attuned = !attuned;
                                        }
                                    >
                                        <Icon name="wand-sparkles" />
                                    </button>
                                }
                            });
                        let equipped_checkbox = view! {
                            <>
                                <input
                                    type="checkbox"
                                    class="entry-equipped"
                                    title=move_tr!("equipped")
                                    prop:checked=equipped
                                    on:change=move |e| {
                                        items_store.write()[idx].equipped = event_target_checked(
                                            &e,
                                        );
                                    }
                                />
                                {attune_toggle}
                            </>
                        }
                            .into_any();
                        let qty_input = view! {
                            <input
                                type="number"
                                class="session-qty-input"
                                min="0"
                                prop:value=qty.to_string()
                                on:input=move |e| {
                                    let Ok(value) = event_target_value(&e).parse() else { return };
                                    items_store.write()[idx].quantity = value;
                                }
                            />
                        }
                            .into_any();
                        let desc_edit = view! {
                            <textarea
                                class="entry-desc"
                                prop:value=desc
                                on:input=move |e| {
                                    items_store.write()[idx].description = event_target_value(&e);
                                }
                            />
                        }
                            .into_any();
                        SessionListItem {
                            name: item.name.clone(),
                            description: String::new(),
                            badge: None,
                            actions: None,
                            name_prefix: Some(equipped_checkbox),
                            name_extra: Some(qty_input),
                            description_view: Some(desc_edit),
                        }
                    })
                    .collect::<Vec<_>>();
                if items.is_empty() {
                    Either::Left(
                        view! { <p class="session-empty">{move_tr!("session-no-items")}</p> },
                    )
                } else {
                    Either::Right(view! { <SessionList items=items /> })
                }
            }}

        </div>
    }
}
