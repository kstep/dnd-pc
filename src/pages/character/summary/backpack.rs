use std::collections::HashSet;

use leptos::{either::Either, prelude::*};
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::toggle_button::ToggleButton,
    model::{Character, CharacterStoreFields, EquipmentStoreFields},
};

#[component]
pub fn BackpackBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let equipment = store.equipment();

    view! {
        <div class="summary-section">
            <h3 class="summary-section-title">{move_tr!("summary-backpack")}</h3>

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
                        <div class="summary-list">
                            {items.into_iter().map(|(idx, name, qty, desc)| {
                                let is_open = Signal::derive(move || expanded.get().contains(&idx));
                                view! {
                                    <div class="summary-list-entry">
                                        <div class="summary-list-row">
                                            <ToggleButton
                                                expanded=is_open
                                                on_toggle=move || expanded.update(|set| { if !set.remove(&idx) { set.insert(idx); } })
                                            />
                                            <span class="summary-list-name">{name}</span>
                                            <span class="summary-list-badge">
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
                                        <Show when=move || is_open.get()>
                                            <textarea
                                                class="summary-item-desc"
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

            // -- Currency --
            <div class="summary-currency">
                <label>{move_tr!("currency")}</label>
                <span>{move || equipment.currency().read().to_string()}</span>
            </div>
        </div>
    }
}
