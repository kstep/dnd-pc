use leptos::prelude::*;
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;
use strum::VariantArray;

use crate::{
    components::{
        enchantment::{ActionsSection, ChargesSection, PassivesSection},
        expr_view::ExprDetails,
        icon::Icon,
        modal::Modal,
    },
    model::{
        Ability, ArmorType, DamageEffect, DamageType, Expr, Item, ItemEffects,
        ItemEffectsStoreFields, ItemKind, ItemStoreFields, Money, Translatable, WeaponCategory,
        Weight,
    },
};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    General,
    Damage,
    Effects,
    Actions,
}

/// Modal for editing a whole `Item`: General (name, quantity, weight, price,
/// kind parameters) and Effects (damage lines + assigns/actions/charges).
/// Edits local drafts; commits via `on_save` only on the Save button.
#[component]
pub fn ItemModal(
    show: RwSignal<bool>,
    #[prop(into)] value: Signal<Item>,
    on_save: Callback<Item>,
) -> impl IntoView {
    let draft = Store::new(Item::default());
    let effects_draft = Store::new(ItemEffects::default());
    let tab = RwSignal::new(Tab::General);

    Effect::new(move || {
        if show.get() {
            let item = value.get_untracked();
            effects_draft.set(item.effects.clone());
            draft.set(item);
            tab.set(Tab::General);
        }
    });

    let save = move |_| {
        let mut item = draft.get_untracked();
        item.effects = effects_draft.get_untracked();
        on_save.run(item);
        show.set(false);
    };

    // Mirrors `TabNav` markup (icon + label in an <a>) so the modal tabs
    // inherit the app tab look, including the mobile label collapse.
    let tab_link = move |target: Tab, icon: &'static str, label: Signal<String>| {
        view! {
            <a
                class="tab-nav-link"
                aria-current=move || (tab.get() == target).then_some("page")
                on:click=move |_| tab.set(target)
            >
                <Icon name=icon size=16 />
                <span class="tab-nav-label">{label}</span>
            </a>
        }
    };

    let is_weapon = move || draft.read().is_weapon();

    view! {
        <Modal show title=move_tr!("item-edit")>
            <nav class="tab-nav">
                {tab_link(Tab::General, "info", move_tr!("item-tab-general"))}
                <Show when=is_weapon>{tab_link(Tab::Damage, "swords", move_tr!("damage"))}</Show>
                {tab_link(Tab::Effects, "sparkles", move_tr!("item-tab-effects"))}
                {tab_link(Tab::Actions, "zap", move_tr!("enchantment-actions"))}
            </nav>
            <div class="modal-body item-modal">
                <Show when=move || tab.get() == Tab::General>
                    <GeneralTab draft />
                </Show>
                <Show when=move || tab.get() == Tab::Damage>
                    <DamageSection draft=effects_draft />
                </Show>
                <Show when=move || tab.get() == Tab::Effects>
                    <PassivesSection draft=effects_draft />
                </Show>
                <Show when=move || tab.get() == Tab::Actions>
                    <ChargesSection draft=effects_draft />
                    <ActionsSection draft=effects_draft />
                </Show>
            </div>
            <div class="modal-actions">
                <button on:click=move |_| show.set(false)>{move_tr!("btn-cancel")}</button>
                <button class="btn-primary" on:click=save>
                    {move_tr!("btn-save")}
                </button>
            </div>
        </Modal>
    }
}

#[component]
fn GeneralTab(draft: Store<Item>) -> impl IntoView {
    // Read-only preview of the formula the kind parameters derive.
    let derived_expr = move || {
        let item = draft.read();
        item.attack_expr().or_else(|| item.ac_expr())
    };

    view! {
        <section class="item-general">
            <label class="field-block">
                <span class="field-label">{move_tr!("item-name")}</span>
                <input
                    type="text"
                    class="entry-name"
                    prop:value=move || draft.name().get()
                    on:change=move |e| draft.name().set(event_target_value(&e))
                />
            </label>
            <label class="field-block">
                <span class="field-label">{move_tr!("description")}</span>
                <textarea
                    class="entry-desc"
                    prop:value=move || draft.description().get()
                    on:change=move |e| draft.description().set(event_target_value(&e))
                />
            </label>
            <div class="option-card-meta">
                <label class="field-inline">
                    <span class="field-label">{move_tr!("qty")}</span>
                    <input
                        type="number"
                        min="0"
                        prop:value=move || draft.quantity().get().to_string()
                        on:change=move |e| {
                            let value = event_target_value(&e).parse::<u32>().unwrap_or_default();
                            draft.quantity().set(value);
                        }
                    />
                </label>
                <label class="field-inline">
                    <span class="field-label">{move_tr!("item-weight")} ", lb"</span>
                    <input
                        type="text"
                        inputmode="decimal"
                        prop:value=move || draft.weight().get().to_string()
                        on:change=move |e| {
                            let Ok(value) = event_target_value(&e).parse::<Weight>() else {
                                return;
                            };
                            draft.weight().set(value);
                        }
                    />
                </label>
                <label class="field-inline">
                    <span class="field-label">{move_tr!("item-price")} ", gp"</span>
                    <input
                        type="text"
                        inputmode="decimal"
                        prop:value=move || draft.price().get().to_gp_str()
                        on:change=move |e| {
                            let Some(value) = Money::from_gp_str(&event_target_value(&e)) else {
                                return;
                            };
                            draft.price().set(value);
                        }
                    />
                </label>
            </div>
            {move || match draft.kind().get() {
                ItemKind::Misc => ().into_any(),
                ItemKind::Weapon { category, ability, magic_bonus } => {
                    view! {
                        <div class="option-card-meta">
                            <WeaponParams draft category ability magic_bonus />
                        </div>
                    }
                        .into_any()
                }
                ItemKind::Armor { armor_type, base_ac } => {
                    view! {
                        <div class="option-card-meta">
                            <ArmorParams draft armor_type base_ac />
                        </div>
                    }
                        .into_any()
                }
            }}
            <label class="field-inline">
                <input
                    type="checkbox"
                    prop:checked=move || draft.requires_attunement().get()
                    on:change=move |e| {
                        draft.requires_attunement().set(event_target_checked(&e));
                    }
                />
                {move_tr!("requires-attunement")}
            </label>
            {move || derived_expr().map(|expr| view! { <ExprDetails expr /> })}
        </section>
    }
}

#[component]
fn WeaponParams(
    draft: Store<Item>,
    category: WeaponCategory,
    ability: Ability,
    magic_bonus: i32,
) -> impl IntoView {
    let i18n = expect_context::<I18n>();

    let set_kind = move |category: WeaponCategory, ability: Ability, magic_bonus: i32| {
        draft.kind().set(ItemKind::Weapon {
            category,
            ability,
            magic_bonus,
        });
    };

    view! {
        <>
            <label class="field-inline">
                <span class="field-label">{move_tr!("weapon-category")}</span>
                <select on:change=move |e| {
                    let index = event_target_value(&e).parse::<u8>().unwrap_or(0);
                    let category = WeaponCategory::try_from(index).unwrap_or_default();
                    set_kind(category, ability, magic_bonus);
                }>
                    {WeaponCategory::VARIANTS
                        .iter()
                        .map(|&variant| {
                            let label = Signal::derive(move || i18n.tr(variant.tr_key()));
                            view! {
                                <option
                                    value=(variant as u8).to_string()
                                    selected=move || variant == category
                                >
                                    {label}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>
            <label class="field-inline">
                <span class="field-label">{move_tr!("ability")}</span>
                <select on:change=move |e| {
                    let index = event_target_value(&e).parse::<u8>().unwrap_or(0);
                    let ability = Ability::try_from(index).unwrap_or_default();
                    set_kind(category, ability, magic_bonus);
                }>
                    {Ability::VARIANTS
                        .iter()
                        .map(|&variant| {
                            let label = Signal::derive(move || i18n.tr(variant.tr_key()));
                            view! {
                                <option
                                    value=(variant as u8).to_string()
                                    selected=move || variant == ability
                                >
                                    {label}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>
            <label class="field-inline">
                <span class="field-label">{move_tr!("magic-bonus")}</span>
                <input
                    type="number"
                    prop:value=magic_bonus.to_string()
                    on:change=move |e| {
                        let value = event_target_value(&e).parse::<i32>().unwrap_or(0);
                        set_kind(category, ability, value);
                    }
                />
            </label>
        </>
    }
}

#[component]
fn ArmorParams(draft: Store<Item>, armor_type: ArmorType, base_ac: u32) -> impl IntoView {
    let i18n = expect_context::<I18n>();

    let set_kind = move |armor_type: ArmorType, base_ac: u32| {
        draft.kind().set(ItemKind::Armor {
            armor_type,
            base_ac,
        });
    };

    view! {
        <>
            <label class="field-inline">
                <span class="field-label">{move_tr!("armor-type")}</span>
                <select on:change=move |e| {
                    let index = event_target_value(&e).parse::<u8>().unwrap_or(0);
                    let armor_type = ArmorType::try_from(index).unwrap_or_default();
                    set_kind(armor_type, base_ac);
                }>
                    {ArmorType::VARIANTS
                        .iter()
                        .map(|&variant| {
                            let label = Signal::derive(move || i18n.tr(variant.tr_key()));
                            view! {
                                <option
                                    value=(variant as u8).to_string()
                                    selected=move || variant == armor_type
                                >
                                    {label}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>
            <label class="field-inline">
                <span class="field-label">{move_tr!("base-ac")}</span>
                <input
                    type="number"
                    min="0"
                    prop:value=base_ac.to_string()
                    on:change=move |e| {
                        let value = event_target_value(&e).parse::<u32>().unwrap_or_default();
                        set_kind(armor_type, value);
                    }
                />
            </label>
        </>
    }
}

/// Damage lines editor (weapon items only).
#[component]
fn DamageSection(draft: Store<ItemEffects>) -> impl IntoView {
    let i18n = expect_context::<I18n>();
    let damage = draft.damage();

    let add = move |_| {
        damage.write().push(DamageEffect::default());
    };

    view! {
        <section class="enchant-section">
            {move || {
                let len = damage.read().len();
                (len > 0)
                    .then(|| {
                        view! {
                            <div class="entry-list">
                                {(0..len)
                                    .map(|index| {
                                        view! {
                                            <DamageRow damage=damage.into() index=index i18n=i18n />
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })
            }} <button class="btn-primary" on:click=add>
                {move_tr!("btn-add-damage")}
            </button>
        </section>
    }
}

#[component]
fn DamageRow(
    damage: reactive_stores::Field<Vec<DamageEffect>>,
    index: usize,
    i18n: I18n,
) -> impl IntoView {
    let read_row = move |f: &dyn Fn(&DamageEffect) -> String| -> String {
        damage.read().get(index).map(f).unwrap_or_default()
    };
    let damage_type = move || damage.read().get(index).and_then(|row| row.damage_type);

    let on_name = move |event: web_sys::Event| {
        let raw = event_target_value(&event);
        damage.update(|list| {
            if let Some(row) = list.get_mut(index) {
                row.name = raw;
            }
        });
    };
    let on_type = move |event: web_sys::Event| {
        let raw = event_target_value(&event);
        let parsed = raw
            .parse::<u8>()
            .ok()
            .and_then(|value| DamageType::try_from(value).ok());
        damage.update(|list| {
            if let Some(row) = list.get_mut(index) {
                row.damage_type = parsed;
            }
        });
    };
    let on_expr = move |event: web_sys::Event| {
        let parsed: Expr = event_target_value(&event).parse().unwrap_or_default();
        damage.update(|list| {
            if let Some(row) = list.get_mut(index) {
                row.expr = parsed;
            }
        });
    };
    let remove = move |_| {
        damage.update(|list| {
            if index < list.len() {
                list.remove(index);
            }
        });
    };

    view! {
        <div class="entry-item">
            <div class="entry-content damage-row">
                <input
                    type="text"
                    class="entry-name"
                    placeholder=move_tr!("effect-name")
                    prop:value=move || read_row(&|row| row.name.clone())
                    on:change=on_name
                />
                <select on:change=on_type>
                    <option value="" selected=move || damage_type().is_none()>
                        "—"
                    </option>
                    {DamageType::VARIANTS
                        .iter()
                        .map(|&variant| {
                            let label = Signal::derive(move || i18n.tr(variant.tr_key()));
                            view! {
                                <option
                                    value=(variant as u8).to_string()
                                    selected=move || damage_type() == Some(variant)
                                >
                                    {label}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
                <div class="entry-actions">
                    <button class="btn-remove" on:click=remove>
                        <Icon name="x" />
                    </button>
                </div>
                <input
                    type="text"
                    class="effect-expr-input"
                    placeholder=move_tr!("effect-expr-required")
                    prop:value=move || read_row(&|row| row.expr.to_string())
                    on:change=on_expr
                />
            </div>
        </div>
    }
}
