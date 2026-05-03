use std::collections::BTreeMap;

use leptos::{either::Either, prelude::*};
use leptos_fluent::{I18n, move_tr};
use reactive_stores::Store;

use crate::{
    components::{
        cast_button::{CastButton, CastOption},
        effects_calc_modal::{
            EffectsCalcInfo, EffectsCalcModal, all_self_effects_diceless, apply_self_effects_now,
            open_calc_modal,
        },
        icon::Icon,
        session_list::{SessionList, SessionListItem},
    },
    effective::EffectiveCharacter,
    model::{Attribute, Character, GearRef, Translatable},
    rules::{ActionDefinition, ChoiceOption, ChoiceOptions},
};

/// Renders all action options from active items / weapons / armors that have
/// an `action` type assigned. Each option becomes a `SessionListItem` with a
/// cast button; clicking decrements `cost` charges and `consumes` quantity,
/// then runs the option's effects through the standard self-cast / calc-modal
/// pipeline.
#[component]
pub fn GearActionsBlock() -> impl IntoView {
    let store = expect_context::<Store<Character>>();
    let eff = expect_context::<EffectiveCharacter>();
    let i18n = expect_context::<I18n>();

    let show_calc = RwSignal::new(false);
    let calc_info = StoredValue::new(None::<EffectsCalcInfo>);

    let cast = move |target: GearRef, opt: &ChoiceOption, gear_name: String| {
        let cost = opt.cost;
        let consumes = opt.consumes;
        let label = opt.label().to_string();
        let effects = opt.effects.clone();

        store.update(|character| match target {
            GearRef::Item(i) => {
                if let Some(item) = character.equipment.items.get_mut(i) {
                    if let Some(ref mut charges) = item.magic.charges {
                        charges.used = (charges.used + cost).min(charges.max);
                    }
                    item.quantity = item.quantity.saturating_sub(consumes);
                }
            }
            GearRef::Weapon(i) => {
                if let Some(weapon) = character.equipment.weapons.get_mut(i) {
                    if let Some(ref mut charges) = weapon.magic.charges {
                        charges.used = (charges.used + cost).min(charges.max);
                    }
                    weapon.quantity = weapon.quantity.saturating_sub(consumes);
                }
            }
            GearRef::Armor(i) => {
                if let Some(armor) = character.equipment.armors.get_mut(i)
                    && let Some(ref mut charges) = armor.magic.charges
                {
                    charges.used = (charges.used + cost).min(charges.max);
                }
            }
        });

        if effects.is_empty() {
            return;
        }

        let mut extra_vars = BTreeMap::new();
        extra_vars.insert(Attribute::Cost, cost as i32);

        let character = store.read_untracked();
        let all_caster = effects.iter().all(|e| e.range.can_target_self());
        if all_caster && all_self_effects_diceless(&effects, &character, &extra_vars) {
            drop(character);
            apply_self_effects_now(
                &effects,
                &label,
                &gear_name,
                &extra_vars,
                &store,
                eff.effects(),
            );
            return;
        }
        drop(character);

        open_calc_modal(
            show_calc,
            calc_info,
            EffectsCalcInfo {
                title: label.clone(),
                effects,
                extra_vars,
                spell_name: label,
                feature_name: gear_name,
            },
        );
    };

    let content = move || {
        let character = store.read();
        let mut items = Vec::new();

        let push_options = |items: &mut Vec<SessionListItem>,
                            target: GearRef,
                            action: &ActionDefinition,
                            gear_name: &str,
                            available_charges: u32,
                            quantity: u32| {
            let ChoiceOptions::List(opts) = &action.options else {
                return;
            };
            for opt in opts.iter() {
                if opt.action.is_none() {
                    continue;
                }
                if opt.cost > available_charges || opt.consumes > quantity {
                    continue;
                }
                let action_type = opt.action;
                let opt_name = if opt.name.is_empty() {
                    action.name.to_string()
                } else {
                    format!("{} — {}", action.name, opt.label())
                };
                let owned_opt = opt.clone();
                let owned_gear_name = gear_name.to_string();

                let badge = {
                    let action_icon = action_type.map(|at| {
                        view! {
                            <span class="entry-badge" title=i18n.tr(at.tr_key())>
                                <Icon name=at.icon_name() />
                            </span>
                        }
                    });
                    let cost_badge = (opt.cost > 0).then(|| {
                        view! {
                            <span class="entry-badge session-choice-cost">
                                {opt.cost} "/" {available_charges}
                            </span>
                        }
                    });
                    let consumes_badge = (opt.consumes > 0).then(|| {
                        view! {
                            <span class="entry-badge">
                                "−" {opt.consumes}
                            </span>
                        }
                    });
                    view! {
                        <>{action_icon}{cost_badge}{consumes_badge}</>
                    }
                    .into_any()
                };

                let cast_owned = move |_: CastOption| {
                    cast(target, &owned_opt, owned_gear_name.clone());
                };
                let cast_button = view! {
                    <CastButton on_cast=Callback::new(cast_owned) />
                }
                .into_any();

                items.push(SessionListItem {
                    name: opt_name,
                    description: opt.description.clone(),
                    badge: Some(badge),
                    actions: Some(cast_button),
                    name_prefix: None,
                    name_extra: None,
                    description_view: None,
                });
            }
        };

        for (i, item) in character.equipment.items.iter().enumerate() {
            if !item.is_active() || item.magic.actions.is_empty() {
                continue;
            }
            let available = item
                .magic
                .charges
                .as_ref()
                .map(|c| c.available())
                .unwrap_or(u32::MAX);
            for action in &item.magic.actions {
                push_options(
                    &mut items,
                    GearRef::Item(i),
                    action,
                    &item.name,
                    available,
                    item.quantity,
                );
            }
        }
        for (i, weapon) in character.equipment.weapons.iter().enumerate() {
            if !weapon.is_active() || weapon.magic.actions.is_empty() {
                continue;
            }
            let available = weapon
                .magic
                .charges
                .as_ref()
                .map(|c| c.available())
                .unwrap_or(u32::MAX);
            for action in &weapon.magic.actions {
                push_options(
                    &mut items,
                    GearRef::Weapon(i),
                    action,
                    &weapon.name,
                    available,
                    weapon.quantity,
                );
            }
        }
        for (i, armor) in character.equipment.armors.iter().enumerate() {
            if !armor.is_active() || armor.magic.actions.is_empty() {
                continue;
            }
            let available = armor
                .magic
                .charges
                .as_ref()
                .map(|c| c.available())
                .unwrap_or(u32::MAX);
            for action in &armor.magic.actions {
                push_options(
                    &mut items,
                    GearRef::Armor(i),
                    action,
                    &armor.name,
                    available,
                    1,
                );
            }
        }

        if items.is_empty() {
            Either::Left(())
        } else {
            Either::Right(view! {
                <div class="session-subsection">
                    <h4 class="session-subsection-title">
                        {move_tr!("session-gear-actions")}
                    </h4>
                    <SessionList items=items />
                </div>
            })
        }
    };

    view! {
        {content}
        <EffectsCalcModal show=show_calc info=calc_info />
    }
}
