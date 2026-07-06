use std::collections::BTreeMap;

use leptos::{either::Either, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::CastButton,
        effects_calc_modal::{EffectsCalcInfo, EffectsCalcModal, open_calc_modal},
        icon::Icon,
        session_list::{SessionList, SessionListItem},
    },
    effective::EffectiveCharacter,
    model::{
        Character, CharacterStoreFields, EffectDefinition, EquipmentStoreFields, Translatable,
    },
};

#[component]
pub fn WeaponsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let i18n = expect_context::<I18n>();
    let items = store.equipment().items();

    let show_calc = RwSignal::new(false);
    let calc_info = StoredValue::new(None::<EffectsCalcInfo>);

    let content = move || {
        let rows = items
            .read()
            .iter()
            .enumerate()
            .filter(|(_, weapon)| weapon.is_weapon() && !weapon.name.is_empty())
            .map(|(idx, weapon)| {
                let total_atk = eff.weapon_attack_bonus(weapon);
                let name_atk = weapon.name.clone();
                let quantity = weapon.quantity;
                let equipped = weapon.equipped;

                let attune_toggle = weapon.requires_attunement.then(|| {
                    view! {
                        <button
                            class="btn-icon attune-toggle"
                            class:attuned=move || items.read()[idx].attuned
                            title=move_tr!("attuned")
                            on:click=move |_| {
                                let attuned = items.read()[idx].attuned;
                                items.write()[idx].attuned = !attuned;
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
                                items.write()[idx].equipped = event_target_checked(&e);
                            }
                        />
                        {attune_toggle}
                    </>
                }
                .into_any();

                let active_effects: Vec<_> = weapon
                    .effects
                    .damage
                    .iter()
                    .filter(|e| !e.expr.is_empty())
                    .collect();
                let has_effects = !active_effects.is_empty();

                let attack_badge = view! {
                    <span class="entry-badge">
                        <Icon name="swords" />
                        " " {move_tr!("attack")} " " {format!("{total_atk:+}")}
                    </span>
                };

                // First effect as inline badge, "…" if more
                let first_badge = active_effects.first().map(|effect| {
                    let icon = effect
                        .damage_type
                        .map(|dt| view! { <Icon name=dt.icon_name() /> });
                    let title = if effect.name.is_empty() {
                        None
                    } else {
                        Some(effect.name.clone())
                    };
                    let expr = effect.expr.to_string();
                    view! { <span class="entry-badge" title=title>{icon}" "{expr}</span> }
                });
                let more = (active_effects.len() > 1)
                    .then(|| view! { <span class="entry-badge">"\u{2026}"</span> });

                // Full list for expandable description
                let description = if active_effects.len() > 1 {
                    active_effects
                        .iter()
                        .map(|effect| {
                            let dt = effect
                                .damage_type
                                .map(|dt| i18n.tr(dt.tr_key()))
                                .unwrap_or_default();
                            let name = &effect.name;
                            let expr = &effect.expr;
                            if name.is_empty() {
                                format!("{dt} {expr}")
                            } else {
                                format!("{dt} {expr} ({name})")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    String::new()
                };

                let cast_button = has_effects.then(|| {
                    let effects: Vec<EffectDefinition> = weapon
                        .effects
                        .damage
                        .iter()
                        .map(EffectDefinition::from)
                        .collect();
                    let title = name_atk.clone();
                    view! {
                        <CastButton on_cast=Callback::new(move |_| {
                            open_calc_modal(
                                show_calc,
                                calc_info,
                                EffectsCalcInfo {
                                    title: title.clone(),
                                    effects: effects.clone(),
                                    extra_vars: BTreeMap::new(),
                                    spell_name: String::new(),
                                    feature_name: String::new(),
                                },
                            );
                        }) />
                    }
                    .into_any()
                });

                let qty_input = view! {
                    <input
                        type="number"
                        class="session-qty-input"
                        min="1"
                        prop:value=quantity.to_string()
                        on:input=move |e| {
                            let Ok(value) = event_target_value(&e).parse::<u32>() else { return };
                            items.write()[idx].quantity = value.max(1);
                        }
                    />
                }
                .into_any();

                SessionListItem {
                    name: name_atk,
                    description,
                    badge: Some(view! { <>{attack_badge}{first_badge}{more}</> }.into_any()),
                    actions: cast_button,
                    name_prefix: Some(equipped_checkbox),
                    name_extra: Some(qty_input),
                    description_view: None,
                }
            })
            .collect::<Vec<_>>();

        let attack_count = eff.attack_count();

        if rows.is_empty() {
            Either::Left(view! {
                <p class="session-empty">{move_tr!("session-no-weapons")}</p>
            })
        } else {
            Either::Right(view! {
                <div class="session-subsection">
                    <h4 class="session-subsection-title">
                        {move_tr!("weapons")}
                        {(attack_count > 1).then(|| view! {
                            <span class="entry-badge">{move_tr!("attack-count")} ": " {attack_count}</span>
                        })}
                    </h4>
                    <SessionList items=rows />
                </div>
            })
        }
    };

    view! {
        {content}
        <EffectsCalcModal show=show_calc info=calc_info />
    }
}
