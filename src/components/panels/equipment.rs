use leptos::prelude::*;
use leptos_fluent::move_tr;
use reactive_stores::Store;

use crate::{
    components::{
        icon::Icon, item_modal::ItemModal, slot_box::SlotBox, toggle_button::ToggleButton,
    },
    model::{
        Ability, Character, CharacterStoreFields, CurrencyStoreFields, DamageEffect,
        EquipmentStoreFields, Item, ItemKind,
    },
};

/// Stable (locale-independent) name used to identify the auto-created
/// damage effect on a new weapon. Kept in English so that ability-change
/// auto-fill keeps matching even after the UI locale is switched.
pub const DAMAGE_EFFECT_NAME: &str = "Damage";

/// Stable in-place sort of only the rows matching `pred`; other kinds keep
/// their positions.
fn sort_kind_by_name(list: &mut [Item], pred: fn(&Item) -> bool) {
    let indexes: Vec<usize> = list
        .iter()
        .enumerate()
        .filter(|(_, item)| pred(item))
        .map(|(index, _)| index)
        .collect();
    let mut picked: Vec<Item> = indexes.iter().map(|&index| list[index].clone()).collect();
    picked.sort_by_key(|item| item.name.to_lowercase());
    for (&index, item) in indexes.iter().zip(picked) {
        list[index] = item;
    }
}

fn is_misc(item: &Item) -> bool {
    !item.is_weapon() && !item.is_armor()
}

#[component]
pub fn EquipmentPanel() -> impl IntoView {
    let store = expect_context::<Store<Character>>();

    let equipment = store.equipment();
    let items = equipment.items();
    let currency = equipment.currency();

    let currency_expanded = RwSignal::new(false);
    let show_cp = move || currency_expanded.get() || currency.cp().get() > 0;
    let show_sp = move || currency_expanded.get() || currency.sp().get() > 0;
    let show_ep = move || currency_expanded.get() || currency.ep().get() > 0;
    let show_pp = move || currency_expanded.get() || currency.pp().get() > 0;

    // Item modal state — references the currently-edited item slot.
    let edit_target = RwSignal::new(None::<usize>);
    let show_editor = RwSignal::new(false);
    let editing_item = Signal::derive(move || {
        edit_target
            .get()
            .and_then(|index| items.read().get(index).cloned())
            .unwrap_or_default()
    });
    let save_item = Callback::new(move |value: Item| {
        let Some(index) = edit_target.get_untracked() else {
            return;
        };
        if index < items.read().len() {
            items.write()[index] = value;
        }
    });

    // Total carried weight vs STR × 15 carrying capacity (PHB).
    let total_weight = move || store.read().equipment.total_weight();
    let capacity = move || store.read().ability_score(Ability::Strength) * 15;
    let overweight = move || (total_weight().0 as u64) > (capacity() as u64) * 100;

    let attuned_count = move || store.read().equipment.attuned_count();
    let attune_max = move || store.read().combat.attunement_max;
    let over_attuned = move || attuned_count() > attune_max() as usize;

    view! {
        <section>
            <div class="section-header">
                <button
                    class="btn-toggle-desc"
                    class:expanded=move || currency_expanded.get()
                    on:click=move |_| currency_expanded.update(|v| *v = !*v)
                />
                <h3
                    class="clickable"
                    on:click=move |_| currency_expanded.update(|v| *v = !*v)
                >
                    {move_tr!("currency")}
                </h3>
            </div>
            <div class="currency-row">
                <Show when=show_pp>
                    <SlotBox label="PP" label_after=true>
                        <input
                            type="number"
                            min="0"
                            prop:value=move || currency.pp().get().to_string()
                            on:change=move |e| {
                                let value = event_target_value(&e).parse::<u32>().unwrap_or_default();
                                currency.pp().set(value);
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
                        on:change=move |e| {
                            let value = event_target_value(&e).parse::<u32>().unwrap_or_default();
                            currency.gp().set(value);
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
                            on:change=move |e| {
                                let value = event_target_value(&e).parse::<u32>().unwrap_or_default();
                                currency.ep().set(value);
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
                            on:change=move |e| {
                                let value = event_target_value(&e).parse::<u32>().unwrap_or_default();
                                currency.sp().set(value);
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
                            on:change=move |e| {
                                let value = event_target_value(&e).parse::<u32>().unwrap_or_default();
                                currency.cp().set(value);
                            }
                        />
                    </SlotBox>
                </Show>
            </div>
            <p class="inventory-weight">
                <span class:overweight=overweight>
                    {move_tr!("inventory-total-weight")} ": "
                    {move || total_weight().to_string()} " / " {capacity} " lb"
                </span>
                " \u{00b7} "
                <span class:overweight=over_attuned>
                    {move_tr!("inventory-attuned")} ": " {attuned_count} " / " {attune_max}
                </span>
            </p>
        </section>

        <GearSection title=move_tr!("weapons") pred=Item::is_weapon make_kind=ItemKind::default_weapon edit_target show_editor />
        <GearSection title=move_tr!("armor") pred=Item::is_armor make_kind=ItemKind::default_armor edit_target show_editor />
        <GearSection title=move_tr!("items") pred=is_misc make_kind=misc_kind edit_target show_editor />


        <ItemModal show=show_editor value=editing_item on_save=save_item />
    }
}

fn misc_kind() -> ItemKind {
    ItemKind::Misc
}

/// One kind-filtered section of the unified item list: sort header, rows
/// (equipped + name + quantity + edit/delete), and an add button.
#[component]
fn GearSection(
    #[prop(into)] title: Signal<String>,
    pred: fn(&Item) -> bool,
    make_kind: fn() -> ItemKind,
    /// Slot index currently edited in the item modal (shared with parent).
    edit_target: RwSignal<Option<usize>>,
    show_editor: RwSignal<bool>,
) -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let items = store.equipment().items();

    let rows = move || {
        items
            .read()
            .iter()
            .enumerate()
            .filter(|(_, item)| pred(item))
            .map(|(i, item)| {
                let name = item.name.clone();
                let qty = item.quantity.to_string();
                let desc = item.description.clone();
                let attune_toggle = item.requires_attunement.then(|| view! {
                    <button
                        class="btn-icon attune-toggle"
                        class:attuned=move || items.read()[i].attuned
                        title=move_tr!("attuned")
                        on:click=move |_| {
                            let attuned = items.read()[i].attuned;
                            items.write()[i].attuned = !attuned;
                        }
                    >
                        <Icon name="wand-sparkles" />
                    </button>
                });
                view! {
                    <div class="entry-item">
                        <ToggleButton />
                        <div class="entry-content">
                            <input
                                type="checkbox"
                                class="entry-equipped"
                                title=move_tr!("equipped")
                                prop:checked=move || items.read()[i].equipped
                                on:change=move |e| {
                                    items.write()[i].equipped = event_target_checked(&e);
                                }
                            />
                            {attune_toggle}
                            <input
                                type="text"
                                class="entry-name"
                                placeholder=move_tr!("item-name")
                                prop:value=name
                                on:change=move |e| {
                                    items.write()[i].name = event_target_value(&e);
                                }
                            />
                            <input
                                type="number"
                                class="short-input"
                                placeholder=move_tr!("qty")
                                min="0"
                                prop:value=qty
                                on:change=move |e| {
                                    let value = event_target_value(&e).parse::<u32>().unwrap_or_default();
                                    items.write()[i].quantity = value;
                                }
                            />
                        </div>
                        <div class="entry-actions">
                            <button
                                class="btn-icon"
                                title=move_tr!("item-edit")
                                on:click=move |_| {
                                    edit_target.set(Some(i));
                                    show_editor.set(true);
                                }
                            >
                                <Icon name="pencil" />
                            </button>
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
    };

    let add_item = move |_| {
        let mut item = Item {
            kind: make_kind(),
            ..Item::default()
        };
        if let ItemKind::Weapon { ability, .. } = item.kind {
            item.effects.damage.push(DamageEffect {
                name: DAMAGE_EFFECT_NAME.into(),
                damage_type: None,
                expr: ItemKind::default_damage_expr_str(ability)
                    .parse()
                    .unwrap_or_default(),
            });
        }
        items.write().push(item);
    };

    view! {
        <section>
            <div class="section-header">
                <h3>{title}</h3>
                <button
                    class="btn-toggle-desc"
                    on:click=move |_| {
                        sort_kind_by_name(&mut items.write(), pred);
                    }
                >
                    <Icon name="arrow-down-a-z" size=16 />
                </button>
            </div>
            <div class="entry-list">{rows}</div>
            <button class="btn-primary" on:click=add_item>
                {move_tr!("btn-add-item")}
            </button>
        </section>
    }
}
